//! Experimental local-only host wrapper handoff. Each attachment owns a unique
//! record, so exiting an older connection cannot erase a replacement's record.
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::PathBuf;

pub(crate) struct AttachmentRecord {
    path: PathBuf,
    value: serde_json::Value,
}

impl AttachmentRecord {
    pub(crate) fn publish(
        client_id: u16,
        connection_id: String,
        server_pid: u32,
    ) -> io::Result<Self> {
        let directory = PathBuf::from(
            std::env::var_os("VERIJ_ZELLIJ_IDENTITY_DIR")
                .ok_or_else(|| io::Error::other("no identity directory"))?,
        );
        if !directory.is_absolute() || !directory.is_dir() {
            return Err(io::Error::other(
                "identity directory must exist and be absolute",
            ));
        }
        if connection_id.is_empty()
            || !connection_id
                .bytes()
                .all(|b| b.is_ascii_hexdigit() || b == b'-')
        {
            return Err(io::Error::other("invalid connection token"));
        }
        let path = directory.join(format!("{}-{}.json", std::process::id(), connection_id));
        let value = serde_json::json!({
            "schema_version": 1, "client_id": client_id, "connection_id": connection_id,
            "client_pid": std::process::id(), "server_pid": server_pid, "attached": true,
        });
        let record = Self { path, value };
        record.write()?;
        Ok(record)
    }

    fn write(&self) -> io::Result<()> {
        let temporary = self.path.with_extension("tmp");
        let result = (|| {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temporary)?;
            writeln!(file, "{}", self.value)?;
            file.sync_all()?;
            std::fs::rename(&temporary, &self.path)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(temporary);
        }
        result
    }
}

impl Drop for AttachmentRecord {
    fn drop(&mut self) {
        self.value["attached"] = serde_json::json!(false);
        // Abrupt process death can leave attached=true. Host wrapper MUST check
        // client/server process birth tokens and server generation before use.
        if let Err(error) = self.write() {
            log::warn!("Could not invalidate attachment record: {error}");
        }
    }
}
