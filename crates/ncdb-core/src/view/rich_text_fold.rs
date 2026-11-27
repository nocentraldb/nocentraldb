//! The view's fold of a rich-text operation (RFC 0013, "Folding" and
//! "Un-folding a rejected change"), with the compiler cfg `ncdb_rich_text`:
//! [`super::fold`] hands it the field's state and keeps the bytes it
//! returns.

use crate::change::Operation;
use crate::field_op::{Encoding, Origin};
use crate::log::OperationParkReason;

/// Folds a rich-text operation into `state`, or places it there as a
/// rejected change's (`placing`): the new state's bytes, or why it parks,
/// and whether it displaces operations or placeholders the state holds,
/// which park or give way when the field is folded again in stamp order.
pub(super) fn rich_text(
    field_type: &crate::rich_text::RichText,
    mut state: crate::rich_text::State,
    operation: &Operation,
    origin: Origin,
    placing: bool,
) -> (Result<Vec<u8>, OperationParkReason>, bool) {
    use crate::field::FieldType;
    use crate::rich_text::Fit;
    let edit = match field_type.decode(operation, origin) {
        Ok(edit) => edit,
        Err(reason) => return (Err(reason), false),
    };
    if placing {
        let displaces = matches!(field_type.fit_placeholder(&state, &edit), Fit::Displaces(_));
        field_type.place(&mut state, &edit);
        return (Ok(field_type.state_bytes(&state)), displaces);
    }
    let displaces = matches!(field_type.fit(&state, &edit), Fit::Displaces(_));
    let folded = field_type
        .try_apply(&mut state, &edit)
        .map(|()| field_type.state_bytes(&state));
    (folded, displaces)
}
