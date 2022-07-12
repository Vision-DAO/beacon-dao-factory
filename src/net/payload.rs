use futures::future::{self, TryFutureExt};
use ipfs_api::{response::Cid, IpfsApi, IpfsClient};
use serde::Serialize;
use std::{
    collections::HashMap,
    fs::File,
    io::{Cursor, Read},
};

use super::error::Error;

/// Represents an entry in an Idea's metadata specifying an executable target
/// of a DAO.
#[derive(Serialize)]
struct IdeaPayload {
    /// JS that loads the module (only for kernel modules)
    loader: String,

    /// WASM payload of the module itself
    module: Vec<u8>,
}

/// Represents metadata attached to a DAO.
#[derive(Serialize)]
struct IdeaMetadata<'a> {
    /// Name of the DAO
    title: &'a str,

    /// Markdown description of the DAO
    description: &'a str,

    /// References to the installed modules
    /// CID's are represented in the IPLD dag-json format as maps with one
    /// entry "/" whose value is the string-encoded CID
    payload: Vec<HashMap<&'static str, String>>,
}

/// Creates a metadata instance using the provided details, returning the CID
/// of the uploaded DAG node.
pub async fn deploy_metadata(
    ipfs: &IpfsClient,
    title: &str,
    description: &str,
    modules: Vec<(File, File)>,
) -> Result<Cid, Error> {
    // Load the JS and WASM specified by each module, and get the CID once
    // they are published to IPFS
    let entries: Vec<Cid> = future::try_join_all(modules.into_iter().map(
        async move |(mut load, mut module)| {
            // Modules have a WASM and JS payload. Load the WASM
            let mut src = Vec::new();
            module.read_to_end(&mut src)?;

            // And load the JavaScript
            let mut loader = Vec::new();
            load.read_to_end(&mut loader)?;

            let module = IdeaPayload {
                loader: String::from_utf8(loader).map_err(|e| Error::Io(Box::new(e)))?,
                module: src,
            };

            // Upload the metadata to IPFS
            ipfs.dag_put(Cursor::new(serde_json::to_string(&module)?))
                .map_ok(|resp| resp.cid)
                .map_err(Error::Ipfs)
                .await
        },
    ))
    .await?;

    // See above explanation: DAG-JSON IPLD format requires that CID's are
    // represented as { "/": CID } maps (weird yea ik)
    let payload: Vec<HashMap<&'static str, String>> = entries
        .into_iter()
        .map(|cid| {
            let mut m = HashMap::new();
            m.insert("/", cid.cid_string);

            m
        })
        .collect();

    let meta = IdeaMetadata {
        title,
        description,
        payload,
    };

    ipfs.dag_put(Cursor::new(serde_json::to_string(&meta)?))
        .await
        .map(|resp| resp.cid)
        .map_err(|e| e.into())
}
