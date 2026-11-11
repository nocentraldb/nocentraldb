//! The database in the browser (RFC 0006, "The browser"; NOC-175): on the
//! Origin Private File System, through the core's `opfs-sahpool` VFS, in a
//! dedicated Web Worker. Opening a database at a name waits for NOC-231
//! (where on OPFS its file lives, and which names are allowed).

#[cfg(test)]
mod tests;
