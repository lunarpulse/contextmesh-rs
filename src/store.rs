//! Transactional embedded-Turso admission and ref persistence.

use std::fmt;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;

use tokio::sync::Mutex;
use turso::transaction::{Transaction, TransactionBehavior};
use turso::{Connection, Database, Value, params};

use crate::error::{StoreError, StoreResult};
use crate::model::{AuthorId, ContextId, EventId, SignedEventV1};

mod bundle;
mod dag;
mod sync;
mod verify;

pub use bundle::*;
pub use dag::*;
pub use sync::*;
pub use verify::*;

/// OA-02 database schema version.
pub const STORE_SCHEMA_VERSION: i64 = 1;
/// Maximum canonical byte length of a local ref or peer name.
pub const MAX_REF_NAME_BYTES: usize = 64;
/// Maximum number of authors accepted in one context provision request.
pub const MAX_AUTHORIZED_AUTHORS: usize = 1_024;

const STORE_SCHEMA_FINGERPRINT: &str = "contextmesh.store.v1.2026-08-16";

const REQUIRED_OBJECTS: &[(&str, &str)] = &[
    ("table", "metadata"),
    ("table", "contexts"),
    ("table", "authorized_authors"),
    ("table", "events"),
    ("table", "parent_edges"),
    ("table", "local_refs"),
    ("table", "remote_refs"),
    ("trigger", "events_no_update"),
    ("trigger", "events_no_delete"),
    ("trigger", "edges_no_update"),
    ("trigger", "edges_no_delete"),
    ("trigger", "authors_no_update"),
    ("trigger", "authors_no_delete"),
    ("trigger", "contexts_transition_only"),
    ("trigger", "contexts_no_delete"),
];

const SCHEMA_SQL: &str = r#"
CREATE TABLE metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
INSERT INTO metadata(key, value) VALUES('schema_version', '1');
INSERT INTO metadata(key, value) VALUES('schema_fingerprint', 'contextmesh.store.v1.2026-08-16');
CREATE TABLE contexts(
  context_id BLOB PRIMARY KEY CHECK(length(context_id)=32),
  expected_genesis_id BLOB NOT NULL CHECK(length(expected_genesis_id)=32),
  genesis_event_id BLOB UNIQUE CHECK(genesis_event_id IS NULL OR length(genesis_event_id)=32),
  state INTEGER NOT NULL CHECK(state IN (0,1))
);
CREATE TABLE authorized_authors(
  context_id BLOB NOT NULL,
  author_id BLOB NOT NULL CHECK(length(author_id)=32),
  PRIMARY KEY(context_id, author_id),
  FOREIGN KEY(context_id) REFERENCES contexts(context_id)
);
CREATE TABLE events(
  event_id BLOB PRIMARY KEY CHECK(length(event_id)=32),
  context_id BLOB NOT NULL CHECK(length(context_id)=32),
  author_id BLOB NOT NULL CHECK(length(author_id)=32),
  kind TEXT NOT NULL,
  canonical_wire BLOB NOT NULL,
  FOREIGN KEY(context_id) REFERENCES contexts(context_id)
);
CREATE TABLE parent_edges(
  child_id BLOB NOT NULL,
  ordinal INTEGER NOT NULL CHECK(ordinal>=0 AND ordinal<64),
  parent_id BLOB NOT NULL,
  PRIMARY KEY(child_id, ordinal),
  UNIQUE(child_id, parent_id),
  FOREIGN KEY(child_id) REFERENCES events(event_id),
  FOREIGN KEY(parent_id) REFERENCES events(event_id)
);
CREATE TABLE local_refs(
  context_id BLOB NOT NULL,
  name TEXT NOT NULL,
  event_id BLOB NOT NULL,
  PRIMARY KEY(context_id, name),
  FOREIGN KEY(context_id) REFERENCES contexts(context_id),
  FOREIGN KEY(event_id) REFERENCES events(event_id)
);
CREATE TABLE remote_refs(
  peer TEXT NOT NULL,
  context_id BLOB NOT NULL,
  name TEXT NOT NULL,
  event_id BLOB NOT NULL,
  PRIMARY KEY(peer, context_id, name),
  FOREIGN KEY(context_id) REFERENCES contexts(context_id),
  FOREIGN KEY(event_id) REFERENCES events(event_id)
);
CREATE TRIGGER events_no_update BEFORE UPDATE ON events BEGIN SELECT RAISE(ABORT,'immutable events'); END;
CREATE TRIGGER events_no_delete BEFORE DELETE ON events BEGIN SELECT RAISE(ABORT,'immutable events'); END;
CREATE TRIGGER edges_no_update BEFORE UPDATE ON parent_edges BEGIN SELECT RAISE(ABORT,'immutable edges'); END;
CREATE TRIGGER edges_no_delete BEFORE DELETE ON parent_edges BEGIN SELECT RAISE(ABORT,'immutable edges'); END;
CREATE TRIGGER authors_no_update BEFORE UPDATE ON authorized_authors BEGIN SELECT RAISE(ABORT,'append-only authors'); END;
CREATE TRIGGER authors_no_delete BEFORE DELETE ON authorized_authors BEGIN SELECT RAISE(ABORT,'append-only authors'); END;
CREATE TRIGGER contexts_transition_only BEFORE UPDATE ON contexts
WHEN NOT (
  OLD.state=0 AND NEW.state=1 AND OLD.genesis_event_id IS NULL
  AND NEW.genesis_event_id IS NOT NULL
  AND OLD.context_id=NEW.context_id
  AND OLD.expected_genesis_id=NEW.expected_genesis_id
)
BEGIN SELECT RAISE(ABORT,'invalid context transition'); END;
CREATE TRIGGER contexts_no_delete BEFORE DELETE ON contexts BEGIN SELECT RAISE(ABORT,'immutable contexts'); END;
"#;

macro_rules! name_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            /// Returns the canonical name text.
            #[must_use]
            pub fn as_str(&self) -> &str { &self.0 }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) }
        }
        impl FromStr for $name {
            type Err = StoreError;
            fn from_str(text: &str) -> StoreResult<Self> {
                validate_name(text)?;
                Ok(Self(text.to_owned()))
            }
        }
    };
}

name_type!(/// A validated local branch name.
    LocalRefName);
name_type!(/// A validated peer namespace name.
    PeerName);

/// Explicit local context trust and initial append-only authorization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextProvision {
    /// Opaque context identifier.
    pub context: ContextId,
    /// Exact zero-parent genesis event expected for this context.
    pub expected_genesis: EventId,
    /// Canonically sorted, unique initial author allowlist.
    pub authorized_authors: Vec<AuthorId>,
}

/// Expected state used for a local-ref compare-and-swap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefExpectation {
    /// The ref must not exist.
    Absent,
    /// The ref must point to this exact event.
    Head(EventId),
}

/// Optional ref movement performed in the event-admission transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RefMutation {
    /// Do not move a local ref.
    None,
    /// Move a local ref only if its current state matches the expectation.
    CompareAndSwap {
        /// Context that owns the local ref.
        context: ContextId,
        /// Canonical local ref name.
        name: LocalRefName,
        /// Required current ref state.
        expected: RefExpectation,
        /// New ref head; OA-02 requires this to be the admitted event.
        new_head: EventId,
    },
}

/// Outcome of an idempotent admission operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionStatus {
    /// A new event was inserted.
    Inserted,
    /// The same event was already stored and no ref retry was detected.
    AlreadyPresent,
    /// The same event and requested new ref head were already committed.
    AlreadyApplied,
}

/// Immutable local-ref query result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalRef {
    /// Context that owns the ref.
    pub context: ContextId,
    /// Local branch name.
    pub name: LocalRefName,
    /// Current immutable event head.
    pub head: EventId,
}

/// Immutable remote-tracking ref query result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteRef {
    /// Peer namespace.
    pub peer: PeerName,
    /// Context that owns the advertised event.
    pub context: ContextId,
    /// Peer branch name.
    pub name: LocalRefName,
    /// Current immutable event head advertised for the peer.
    pub head: EventId,
}

/// Asynchronous local embedded-Turso store.
#[derive(Clone)]
pub struct Store {
    database: Database,
    write_gate: Arc<Mutex<()>>,
}

impl Store {
    /// Opens or creates a file-backed store and migrates it to schema v1.
    pub async fn open(path: impl AsRef<Path>) -> StoreResult<Self> {
        let text = path
            .as_ref()
            .to_str()
            .ok_or(StoreError::DatabaseUnavailable)?;
        let database = turso::Builder::new_local(text)
            .build()
            .await
            .map_err(map_db)?;
        let store = Self {
            database,
            write_gate: Arc::new(Mutex::new(())),
        };
        store.migrate().await?;
        Ok(store)
    }

    /// Atomically provisions a pending context and its initial local allowlist.
    pub async fn provision_context(&self, provision: ContextProvision) -> StoreResult<()> {
        validate_authors(&provision.authorized_authors)?;
        self.write(move |tx| Box::pin(async move {
            if let Some((expected, _state)) = context_row(tx, provision.context).await? {
                let authors = author_rows(tx, provision.context).await?;
                if expected == provision.expected_genesis && authors == provision.authorized_authors { return Ok(()); }
                return Err(StoreError::ContextProvisionMismatch);
            }
            tx.execute(
                "INSERT INTO contexts(context_id,expected_genesis_id,genesis_event_id,state) VALUES(?1,?2,NULL,0)",
                params![provision.context.to_bytes().to_vec(), provision.expected_genesis.to_bytes().to_vec()],
            ).await.map_err(map_db)?;
            for author in provision.authorized_authors {
                tx.execute(
                    "INSERT INTO authorized_authors(context_id,author_id) VALUES(?1,?2)",
                    params![provision.context.to_bytes().to_vec(), author.to_bytes().to_vec()],
                ).await.map_err(map_db)?;
            }
            Ok(())
        })).await
    }

    /// Adds an author to a provisioned context's append-only local allowlist.
    pub async fn authorize_author(
        &self,
        context: ContextId,
        author: AuthorId,
    ) -> StoreResult<bool> {
        self.write(move |tx| {
            Box::pin(async move {
                if context_row(tx, context).await?.is_none() {
                    return Err(StoreError::ContextUnknown);
                }
                let changed = tx.execute(
                "INSERT OR IGNORE INTO authorized_authors(context_id,author_id) VALUES(?1,?2)",
                params![context.to_bytes().to_vec(), author.to_bytes().to_vec()],
            ).await.map_err(map_db)?;
                Ok(changed == 1)
            })
        })
        .await
    }

    /// Strictly parses wire bytes and admits the resulting verified event.
    pub async fn admit_wire(
        &self,
        wire: &[u8],
        mutation: RefMutation,
    ) -> StoreResult<AdmissionStatus> {
        let event = SignedEventV1::from_wire(wire).map_err(StoreError::Contract)?;
        self.admit(&event, mutation).await
    }

    /// Validates and atomically admits an event with an optional local-ref CAS.
    pub async fn admit(
        &self,
        event: &SignedEventV1,
        mutation: RefMutation,
    ) -> StoreResult<AdmissionStatus> {
        event.verify().map_err(StoreError::Contract)?;
        let wire = event.to_wire().map_err(StoreError::Contract)?;
        let verified_event = SignedEventV1::from_wire(&wire).map_err(StoreError::Contract)?;
        let id = event.event_id();
        let body = event.body();
        let context = body.context();
        let author = body.author();
        let kind = body.kind().to_owned();
        let parents = body.parents().to_vec();
        if let RefMutation::CompareAndSwap {
            context: ref_context,
            new_head,
            ..
        } = &mutation
            && (*ref_context != context || *new_head != id)
        {
            return Err(StoreError::RefMutationMismatch);
        }
        self.write(move |tx| Box::pin(async move {
            let (expected_genesis, state) = context_row(tx, context).await?.ok_or(StoreError::ContextUnknown)?;
            let is_genesis = id == expected_genesis
                && kind == "context.genesis"
                && parents.is_empty();
            if state == 0 {
                if !is_genesis {
                    return Err(StoreError::GenesisMismatch);
                }
            } else if !is_genesis && (parents.is_empty() || kind == "context.genesis") {
                return Err(StoreError::GenesisMismatch);
            }
            if !is_authorized(tx, context, author).await? { return Err(StoreError::UnauthorizedAuthor); }
            for parent in &parents {
                let parent_context = authoritative_event_context(tx, *parent).await?.ok_or(StoreError::ParentMissing(*parent))?;
                if parent_context != context { return Err(StoreError::ParentContextMismatch(*parent)); }
            }

            let existing = event_wire(tx, id).await?;
            if state == 1 && is_genesis && existing.is_none() {
                return Err(StoreError::CorruptStorage);
            }
            let inserted = match existing {
                None => {
                    tx.execute(
                        "INSERT INTO events(event_id,context_id,author_id,kind,canonical_wire) VALUES(?1,?2,?3,?4,?5)",
                        params![id.to_bytes().to_vec(), context.to_bytes().to_vec(), author.to_bytes().to_vec(), kind, wire.clone()],
                    ).await.map_err(map_db)?;
                    for (ordinal, parent) in parents.iter().enumerate() {
                        tx.execute(
                            "INSERT INTO parent_edges(child_id,ordinal,parent_id) VALUES(?1,?2,?3)",
                            params![id.to_bytes().to_vec(), i64::try_from(ordinal).map_err(|_| StoreError::LimitExceeded)?, parent.to_bytes().to_vec()],
                        ).await.map_err(map_db)?;
                    }
                    true
                }
                Some(stored) if stored == wire => {
                    validate_stored_event(tx, &verified_event).await?;
                    false
                }
                Some(_) => return Err(StoreError::EventCollision),
            };

            if state == 0 {
                let changed = tx.execute(
                    "UPDATE contexts SET genesis_event_id=?1,state=1 WHERE context_id=?2 AND state=0 AND expected_genesis_id=?1",
                    params![id.to_bytes().to_vec(), context.to_bytes().to_vec()],
                ).await.map_err(map_db)?;
                if changed != 1 { return Err(StoreError::GenesisMismatch); }
            }

            let already_applied = apply_ref_mutation(tx, id, mutation).await?;
            Ok(if already_applied { AdmissionStatus::AlreadyApplied } else if inserted { AdmissionStatus::Inserted } else { AdmissionStatus::AlreadyPresent })
        })).await
    }

    /// Returns a stored event after reparsing and strict cryptographic verification.
    pub async fn event(&self, id: EventId) -> StoreResult<Option<SignedEventV1>> {
        let conn = self.connection().await?;
        match event_wire(&conn, id).await? {
            Some(wire) => {
                let event =
                    SignedEventV1::from_wire(&wire).map_err(|_| StoreError::CorruptStorage)?;
                validate_stored_event(&conn, &event).await?;
                Ok(Some(event))
            }
            None => Ok(None),
        }
    }

    /// Returns the current local-ref head.
    pub async fn local_ref(
        &self,
        context: ContextId,
        name: &LocalRefName,
    ) -> StoreResult<Option<EventId>> {
        let conn = self.connection().await?;
        let head = query_optional_id(
            &conn,
            "SELECT event_id FROM local_refs WHERE context_id=?1 AND name=?2",
            params![context.to_bytes().to_vec(), name.as_str()],
        )
        .await?;
        if let Some(head) = head
            && authoritative_event_context(&conn, head).await? != Some(context)
        {
            return Err(StoreError::CorruptStorage);
        }
        Ok(head)
    }

    /// Lists local refs in canonical name order.
    pub async fn list_local_refs(&self, context: ContextId) -> StoreResult<Vec<LocalRef>> {
        let conn = self.connection().await?;
        let mut rows = conn
            .query(
                "SELECT name,event_id FROM local_refs WHERE context_id=?1 ORDER BY name",
                params![context.to_bytes().to_vec()],
            )
            .await
            .map_err(map_db)?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(map_db)? {
            let name: String = row.get(0).map_err(|_| StoreError::CorruptStorage)?;
            let head = id_value(row.get_value(1).map_err(|_| StoreError::CorruptStorage)?)?;
            if authoritative_event_context(&conn, head).await? != Some(context) {
                return Err(StoreError::CorruptStorage);
            }
            out.push(LocalRef {
                context,
                name: name.parse()?,
                head,
            });
        }
        Ok(out)
    }

    /// Lists remote refs, optionally restricted to one peer, in canonical order.
    pub async fn list_remote_refs(
        &self,
        peer: Option<&PeerName>,
        context: ContextId,
    ) -> StoreResult<Vec<RemoteRef>> {
        let conn = self.connection().await?;
        let (sql, args): (&str, Vec<Value>) = match peer {
            Some(peer) => (
                "SELECT peer,name,event_id FROM remote_refs WHERE context_id=?1 AND peer=?2 ORDER BY peer,name",
                vec![context.to_bytes().to_vec().into(), peer.as_str().into()],
            ),
            None => (
                "SELECT peer,name,event_id FROM remote_refs WHERE context_id=?1 ORDER BY peer,name",
                vec![context.to_bytes().to_vec().into()],
            ),
        };
        let mut rows = conn.query(sql, args).await.map_err(map_db)?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(map_db)? {
            let peer: String = row.get(0).map_err(|_| StoreError::CorruptStorage)?;
            let name: String = row.get(1).map_err(|_| StoreError::CorruptStorage)?;
            let head = id_value(row.get_value(2).map_err(|_| StoreError::CorruptStorage)?)?;
            if authoritative_event_context(&conn, head).await? != Some(context) {
                return Err(StoreError::CorruptStorage);
            }
            out.push(RemoteRef {
                peer: peer.parse()?,
                context,
                name: name.parse()?,
                head,
            });
        }
        Ok(out)
    }

    /// Records one explicit remote-tracking ref without changing any local ref.
    pub async fn set_remote_ref(
        &self,
        peer: PeerName,
        context: ContextId,
        name: LocalRefName,
        head: EventId,
    ) -> StoreResult<()> {
        self.write(move |tx| Box::pin(async move {
            let event_context = authoritative_event_context(tx, head).await?.ok_or(StoreError::ParentMissing(head))?;
            if event_context != context { return Err(StoreError::ParentContextMismatch(head)); }
            tx.execute(
                "INSERT INTO remote_refs(peer,context_id,name,event_id) VALUES(?1,?2,?3,?4) ON CONFLICT(peer,context_id,name) DO UPDATE SET event_id=excluded.event_id",
                params![peer.as_str(), context.to_bytes().to_vec(), name.as_str(), head.to_bytes().to_vec()],
            ).await.map_err(map_db)?;
            Ok(())
        })).await
    }

    async fn migrate(&self) -> StoreResult<()> {
        let _guard = self.write_gate.lock().await;
        let mut conn = self.connection().await?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await
            .map_err(map_db)?;
        let result = async {
            let table_count = scalar_i64(&tx, "SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'", ()).await?;
            if table_count == 0 {
                tx.execute_batch(SCHEMA_SQL).await.map_err(|_| StoreError::MigrationFailed)?;
                verify_objects(&tx).await?;
            } else {
                let version = schema_version(&tx).await?;
                if version > STORE_SCHEMA_VERSION { return Err(StoreError::NewerSchema); }
                if version != STORE_SCHEMA_VERSION { return Err(StoreError::MigrationFailed); }
                verify_objects(&tx).await?;
            }
            Ok(())
        }.await;
        finish_transaction(tx, result).await
    }

    async fn connection(&self) -> StoreResult<Connection> {
        let conn = self.database.connect().map_err(map_db)?;
        enable_foreign_keys(&conn).await?;
        Ok(conn)
    }

    async fn write<T, F>(&self, operation: F) -> StoreResult<T>
    where
        F: for<'a> FnOnce(
            &'a Transaction<'a>,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = StoreResult<T>> + 'a>,
        >,
    {
        let _guard = self.write_gate.lock().await;
        let mut conn = self.connection().await?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .await
            .map_err(map_db)?;
        let result = operation(&tx).await;
        finish_transaction(tx, result).await
    }
}

fn validate_name(text: &str) -> StoreResult<()> {
    let bytes = text.as_bytes();
    if bytes.is_empty() || bytes.len() > MAX_REF_NAME_BYTES || !bytes[0].is_ascii_lowercase() {
        return Err(StoreError::InvalidRefName);
    }
    let mut separator = false;
    for byte in &bytes[1..] {
        if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            separator = false;
        } else if matches!(byte, b'.' | b'_' | b'-') && !separator {
            separator = true;
        } else {
            return Err(StoreError::InvalidRefName);
        }
    }
    if separator {
        return Err(StoreError::InvalidRefName);
    }
    Ok(())
}

fn validate_authors(authors: &[AuthorId]) -> StoreResult<()> {
    if authors.len() > MAX_AUTHORIZED_AUTHORS {
        return Err(StoreError::LimitExceeded);
    }
    if authors.is_empty() {
        return Err(StoreError::UnauthorizedAuthor);
    }
    if authors
        .windows(2)
        .any(|pair| pair[0].to_string() >= pair[1].to_string())
    {
        return Err(StoreError::ContextProvisionMismatch);
    }
    Ok(())
}

async fn enable_foreign_keys(conn: &Connection) -> StoreResult<()> {
    conn.execute("PRAGMA foreign_keys=ON", ())
        .await
        .map_err(map_db)?;
    if scalar_i64(conn, "PRAGMA foreign_keys", ()).await? != 1 {
        return Err(StoreError::MigrationFailed);
    }
    Ok(())
}

async fn finish_transaction<T>(tx: Transaction<'_>, result: StoreResult<T>) -> StoreResult<T> {
    match result {
        Ok(value) => {
            tx.commit()
                .await
                .map_err(|_| StoreError::IndeterminateCommit)?;
            Ok(value)
        }
        Err(error) => {
            tx.rollback().await.map_err(map_db)?;
            Err(error)
        }
    }
}

async fn schema_version(conn: &Connection) -> StoreResult<i64> {
    let exists = scalar_i64(
        conn,
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='metadata'",
        (),
    )
    .await?;
    if exists != 1 {
        return Err(StoreError::MigrationFailed);
    }
    let mut rows = conn
        .query("SELECT value FROM metadata WHERE key='schema_version'", ())
        .await
        .map_err(map_db)?;
    let row = rows
        .next()
        .await
        .map_err(map_db)?
        .ok_or(StoreError::MigrationFailed)?;
    let value: String = row.get(0).map_err(|_| StoreError::MigrationFailed)?;
    if rows.next().await.map_err(map_db)?.is_some() {
        return Err(StoreError::MigrationFailed);
    }
    value.parse().map_err(|_| StoreError::MigrationFailed)
}

async fn verify_objects(conn: &Connection) -> StoreResult<()> {
    let mut fingerprint_rows = conn
        .query(
            "SELECT value FROM metadata WHERE key='schema_fingerprint'",
            (),
        )
        .await
        .map_err(|_| StoreError::MigrationFailed)?;
    let fingerprint: String = fingerprint_rows
        .next()
        .await
        .map_err(|_| StoreError::MigrationFailed)?
        .ok_or(StoreError::MigrationFailed)?
        .get(0)
        .map_err(|_| StoreError::MigrationFailed)?;
    if fingerprint != STORE_SCHEMA_FINGERPRINT
        || fingerprint_rows
            .next()
            .await
            .map_err(|_| StoreError::MigrationFailed)?
            .is_some()
    {
        return Err(StoreError::MigrationFailed);
    }
    for (kind, name) in REQUIRED_OBJECTS {
        let mut rows = conn
            .query(
                "SELECT count(*) FROM sqlite_master WHERE type=?1 AND name=?2",
                params![*kind, *name],
            )
            .await
            .map_err(map_db)?;
        let row = rows
            .next()
            .await
            .map_err(map_db)?
            .ok_or(StoreError::MigrationFailed)?;
        let count: i64 = row.get(0).map_err(|_| StoreError::MigrationFailed)?;
        if count != 1 {
            return Err(StoreError::MigrationFailed);
        }
    }
    let mut failures = conn
        .query("PRAGMA foreign_key_check", ())
        .await
        .map_err(|_| StoreError::MigrationFailed)?;
    if failures
        .next()
        .await
        .map_err(|_| StoreError::MigrationFailed)?
        .is_some()
    {
        return Err(StoreError::CorruptStorage);
    }
    Ok(())
}

async fn context_row(conn: &Connection, context: ContextId) -> StoreResult<Option<(EventId, i64)>> {
    let mut rows = conn
        .query(
            "SELECT expected_genesis_id,genesis_event_id,state FROM contexts WHERE context_id=?1",
            params![context.to_bytes().to_vec()],
        )
        .await
        .map_err(map_db)?;
    let Some(row) = rows.next().await.map_err(map_db)? else {
        return Ok(None);
    };
    let expected = id_value(row.get_value(0).map_err(|_| StoreError::CorruptStorage)?)?;
    let genesis = match row.get_value(1).map_err(|_| StoreError::CorruptStorage)? {
        Value::Null => None,
        value => Some(id_value(value)?),
    };
    let state: i64 = row.get(2).map_err(|_| StoreError::CorruptStorage)?;
    if !matches!(state, 0 | 1)
        || (state == 0 && genesis.is_some())
        || (state == 1 && genesis != Some(expected))
        || rows.next().await.map_err(map_db)?.is_some()
    {
        return Err(StoreError::CorruptStorage);
    }
    Ok(Some((expected, state)))
}

async fn author_rows(conn: &Connection, context: ContextId) -> StoreResult<Vec<AuthorId>> {
    let mut rows = conn
        .query(
            "SELECT author_id FROM authorized_authors WHERE context_id=?1 ORDER BY author_id",
            params![context.to_bytes().to_vec()],
        )
        .await
        .map_err(map_db)?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await.map_err(map_db)? {
        out.push(author_value(
            row.get_value(0).map_err(|_| StoreError::CorruptStorage)?,
        )?);
    }
    out.sort_by_key(ToString::to_string);
    Ok(out)
}

async fn is_authorized(
    conn: &Connection,
    context: ContextId,
    author: AuthorId,
) -> StoreResult<bool> {
    Ok(scalar_i64(
        conn,
        "SELECT count(*) FROM authorized_authors WHERE context_id=?1 AND author_id=?2",
        params![context.to_bytes().to_vec(), author.to_bytes().to_vec()],
    )
    .await?
        == 1)
}

async fn authoritative_event_context(
    conn: &Connection,
    id: EventId,
) -> StoreResult<Option<ContextId>> {
    let Some(wire) = event_wire(conn, id).await? else {
        return Ok(None);
    };
    let event = SignedEventV1::from_wire(&wire).map_err(|_| StoreError::CorruptStorage)?;
    if event.event_id() != id {
        return Err(StoreError::CorruptStorage);
    }
    Ok(Some(event.body().context()))
}

async fn event_wire(conn: &Connection, id: EventId) -> StoreResult<Option<Vec<u8>>> {
    let mut rows = conn
        .query(
            "SELECT canonical_wire FROM events WHERE event_id=?1",
            params![id.to_bytes().to_vec()],
        )
        .await
        .map_err(map_db)?;
    let Some(row) = rows.next().await.map_err(map_db)? else {
        return Ok(None);
    };
    let wire = blob_value(row.get_value(0).map_err(|_| StoreError::CorruptStorage)?)?;
    if rows.next().await.map_err(map_db)?.is_some() {
        return Err(StoreError::CorruptStorage);
    }
    Ok(Some(wire))
}

async fn validate_stored_event(conn: &Connection, event: &SignedEventV1) -> StoreResult<()> {
    let mut rows = conn
        .query(
            "SELECT context_id,author_id,kind,canonical_wire FROM events WHERE event_id=?1",
            params![event.event_id().to_bytes().to_vec()],
        )
        .await
        .map_err(map_db)?;
    let row = rows
        .next()
        .await
        .map_err(map_db)?
        .ok_or(StoreError::CorruptStorage)?;
    let context = context_value(row.get_value(0).map_err(|_| StoreError::CorruptStorage)?)?;
    let author = author_value(row.get_value(1).map_err(|_| StoreError::CorruptStorage)?)?;
    let kind: String = row.get(2).map_err(|_| StoreError::CorruptStorage)?;
    let wire = blob_value(row.get_value(3).map_err(|_| StoreError::CorruptStorage)?)?;
    if rows.next().await.map_err(map_db)?.is_some()
        || context != event.body().context()
        || author != event.body().author()
        || kind != event.body().kind()
        || wire != event.to_wire().map_err(StoreError::Contract)?
        || edge_rows(conn, event.event_id()).await? != event.body().parents()
    {
        return Err(StoreError::CorruptStorage);
    }
    Ok(())
}

async fn edge_rows(conn: &Connection, id: EventId) -> StoreResult<Vec<EventId>> {
    let mut rows = conn
        .query(
            "SELECT parent_id FROM parent_edges WHERE child_id=?1 ORDER BY ordinal",
            params![id.to_bytes().to_vec()],
        )
        .await
        .map_err(map_db)?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await.map_err(map_db)? {
        out.push(id_value(
            row.get_value(0).map_err(|_| StoreError::CorruptStorage)?,
        )?);
    }
    Ok(out)
}

async fn apply_ref_mutation(
    conn: &Connection,
    event: EventId,
    mutation: RefMutation,
) -> StoreResult<bool> {
    let RefMutation::CompareAndSwap {
        context,
        name,
        expected,
        new_head,
    } = mutation
    else {
        return Ok(false);
    };
    let current = query_optional_id(
        conn,
        "SELECT event_id FROM local_refs WHERE context_id=?1 AND name=?2",
        params![context.to_bytes().to_vec(), name.as_str()],
    )
    .await?;
    if current == Some(new_head) {
        return Ok(true);
    }
    match (expected, current) {
        (RefExpectation::Absent, None) => {
            conn.execute(
                "INSERT INTO local_refs(context_id,name,event_id) VALUES(?1,?2,?3)",
                params![
                    context.to_bytes().to_vec(),
                    name.as_str(),
                    event.to_bytes().to_vec()
                ],
            )
            .await
            .map_err(map_db)?;
            Ok(false)
        }
        (RefExpectation::Head(expected), Some(current)) if expected == current => {
            let changed = conn.execute("UPDATE local_refs SET event_id=?1 WHERE context_id=?2 AND name=?3 AND event_id=?4", params![event.to_bytes().to_vec(), context.to_bytes().to_vec(), name.as_str(), expected.to_bytes().to_vec()]).await.map_err(map_db)?;
            if changed != 1 {
                return Err(StoreError::StaleHead {
                    current: query_optional_id(
                        conn,
                        "SELECT event_id FROM local_refs WHERE context_id=?1 AND name=?2",
                        params![context.to_bytes().to_vec(), name.as_str()],
                    )
                    .await?,
                });
            }
            Ok(false)
        }
        (_, current) => Err(StoreError::StaleHead { current }),
    }
}

async fn query_optional_id(
    conn: &Connection,
    sql: &str,
    arguments: impl turso::IntoParams,
) -> StoreResult<Option<EventId>> {
    let mut rows = conn.query(sql, arguments).await.map_err(map_db)?;
    let Some(row) = rows.next().await.map_err(map_db)? else {
        return Ok(None);
    };
    let id = id_value(row.get_value(0).map_err(|_| StoreError::CorruptStorage)?)?;
    if rows.next().await.map_err(map_db)?.is_some() {
        return Err(StoreError::CorruptStorage);
    }
    Ok(Some(id))
}

async fn scalar_i64(
    conn: &Connection,
    sql: &str,
    arguments: impl turso::IntoParams,
) -> StoreResult<i64> {
    let mut rows = conn.query(sql, arguments).await.map_err(map_db)?;
    let row = rows
        .next()
        .await
        .map_err(map_db)?
        .ok_or(StoreError::CorruptStorage)?;
    let value: i64 = row.get(0).map_err(|_| StoreError::CorruptStorage)?;
    if rows.next().await.map_err(map_db)?.is_some() {
        return Err(StoreError::CorruptStorage);
    }
    Ok(value)
}

fn blob_value(value: Value) -> StoreResult<Vec<u8>> {
    match value {
        Value::Blob(bytes) => Ok(bytes),
        _ => Err(StoreError::CorruptStorage),
    }
}
fn exact_32(value: Value) -> StoreResult<[u8; 32]> {
    blob_value(value)?
        .try_into()
        .map_err(|_| StoreError::CorruptStorage)
}
fn id_value(value: Value) -> StoreResult<EventId> {
    Ok(EventId::from_bytes(exact_32(value)?))
}
fn context_value(value: Value) -> StoreResult<ContextId> {
    Ok(ContextId::from_bytes(exact_32(value)?))
}
fn author_value(value: Value) -> StoreResult<AuthorId> {
    Ok(AuthorId::from_bytes(exact_32(value)?))
}

fn map_db(error: turso::Error) -> StoreError {
    match error {
        turso::Error::Corrupt(_) | turso::Error::NotAdb(_) => StoreError::CorruptStorage,
        _ => StoreError::DatabaseUnavailable,
    }
}
