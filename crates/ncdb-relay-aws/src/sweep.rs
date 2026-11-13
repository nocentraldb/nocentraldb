//! The orphan sweep in bounded runs (NOC-293). A run of the relay's Lambda
//! has a 29-second timeout, and a whole pass over `frames/` can take longer
//! once the bucket holds many frames. So a run stops when its
//! [`SweepBudget`] is spent and stores where it got to, the last key it
//! visited; the next run continues after that key. When a run reaches the
//! end of the listing, the pass is complete, and no new pass starts until
//! [`PASS_EVERY`] after it.
//!
//! The cursor is one reserved item in `spaces`, under [`SWEEP_ITEM`], as
//! the relay epoch is (pending the owner's NOC-306). [`load`] and [`save`]
//! are the only code that knows where it lives.

use std::time::Instant;

use aws_sdk_dynamodb::types::AttributeValue;
use ncdb_relay::store::StoreError;

use crate::item::{b, failed, get_s, n};
use crate::{Aws, SWEEP_ITEM, append};

/// How long after a completed pass the next one starts at the earliest:
/// 23 hours, so a pass completed at a scheduled hour one day starts again
/// at that hour the next, whatever the schedule's jitter.
pub const PASS_EVERY: u64 = 23 * 60 * 60 * 1000;

/// When a run stops: at a deadline, after a number of visited objects, or
/// whichever comes first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SweepBudget {
    deadline: Option<Instant>,
    visits: Option<u32>,
}

impl SweepBudget {
    /// A run that stops at `deadline`.
    pub fn until(deadline: Instant) -> SweepBudget {
        SweepBudget {
            deadline: Some(deadline),
            visits: None,
        }
    }

    /// A run that stops after `visits` objects, for tests.
    pub fn visits(visits: u32) -> SweepBudget {
        SweepBudget {
            deadline: None,
            visits: Some(visits),
        }
    }

    fn spent(&self, visited: u32) -> bool {
        self.visits.is_some_and(|max| visited >= max)
            || self.deadline.is_some_and(|at| Instant::now() >= at)
    }
}

/// How a run ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SweepOutcome {
    /// The budget was spent; the next run continues where this one stopped.
    Paused,
    /// The run reached the end of `frames/`: the pass is complete.
    Completed,
    /// A pass completed less than [`PASS_EVERY`] ago, and none is under
    /// way: the run did nothing.
    Skipped,
}

/// What one run did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SweepRun {
    /// The objects it looked at.
    pub visited: u32,
    /// The orphans it deleted.
    pub deleted: u32,
    /// How it ended.
    pub outcome: SweepOutcome,
}

/// Where a pass stands.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Cursor {
    /// The last key a paused run visited; none when no pass is under way.
    start_after: Option<String>,
    /// When the last pass completed, in milliseconds since the Unix epoch.
    last_pass_at: Option<u64>,
}

async fn load(aws: &Aws) -> Result<Cursor, StoreError> {
    let out = aws
        .db
        .get_item()
        .table_name(&aws.names.spaces)
        .key("space", b(SWEEP_ITEM))
        .consistent_read(true)
        .send()
        .await
        .map_err(failed("read the sweep's cursor"))?;
    let Some(item) = out.item else {
        return Ok(Cursor::default());
    };
    let last_pass_at = match item.get("last_pass_at") {
        Some(AttributeValue::N(number)) => number.parse().ok(),
        _ => None,
    };
    Ok(Cursor {
        start_after: get_s(&item, "start_after").map(str::to_owned),
        last_pass_at,
    })
}

async fn save(aws: &Aws, cursor: &Cursor) -> Result<(), StoreError> {
    let mut put = aws
        .db
        .put_item()
        .table_name(&aws.names.spaces)
        .item("space", b(SWEEP_ITEM));
    if let Some(key) = &cursor.start_after {
        put = put.item("start_after", AttributeValue::S(key.clone()));
    }
    if let Some(at) = cursor.last_pass_at {
        put = put.item("last_pass_at", n(at));
    }
    put.send()
        .await
        .map_err(failed("store the sweep's cursor"))?;
    Ok(())
}

/// One bounded run of the sweep at `now`: it deletes the objects under
/// `frames/` last modified before `cutoff` that no change holds, from where
/// the last run stopped, until `budget` is spent or the listing ends.
pub async fn run(
    aws: &Aws,
    cutoff: u64,
    now: u64,
    budget: SweepBudget,
) -> Result<SweepRun, StoreError> {
    let cursor = load(aws).await?;
    let mut run = SweepRun {
        visited: 0,
        deleted: 0,
        outcome: SweepOutcome::Completed,
    };
    if cursor.start_after.is_none()
        && cursor
            .last_pass_at
            .is_some_and(|at| now < at.saturating_add(PASS_EVERY))
    {
        run.outcome = SweepOutcome::Skipped;
        return Ok(run);
    }
    let mut list = aws
        .s3
        .list_objects_v2()
        .bucket(&aws.names.bucket)
        .prefix("frames/");
    if let Some(key) = &cursor.start_after {
        list = list.start_after(key);
    }
    let mut pages = list.into_paginator().send();
    let mut last: Option<String> = cursor.start_after.clone();
    while let Some(page) = pages.next().await {
        let page = page.map_err(failed("list frames in S3"))?;
        for object in page.contents() {
            if budget.spent(run.visited) {
                run.outcome = SweepOutcome::Paused;
                let paused = Cursor {
                    start_after: last,
                    last_pass_at: cursor.last_pass_at,
                };
                save(aws, &paused).await?;
                return Ok(run);
            }
            run.visited += 1;
            if append::visit(aws, object, cutoff).await? {
                run.deleted += 1;
            }
            last = object.key().map(str::to_owned);
        }
    }
    let completed = Cursor {
        start_after: None,
        last_pass_at: Some(now),
    };
    save(aws, &completed).await?;
    Ok(run)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn a_budget_of_visits_is_spent_after_that_many() {
        let budget = SweepBudget::visits(3);
        assert!(!budget.spent(0));
        assert!(!budget.spent(2));
        assert!(budget.spent(3));
    }

    #[test]
    fn a_budget_until_a_deadline_is_spent_once_it_passed() {
        assert!(!SweepBudget::until(Instant::now() + Duration::from_secs(60)).spent(1_000));
        assert!(SweepBudget::until(Instant::now()).spent(0));
    }

    #[test]
    fn a_new_pass_waits_23_hours() {
        assert_eq!(PASS_EVERY, 23 * 60 * 60 * 1000);
    }
}
