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
#[derive(Serialize, Debug)]
struct IdeaPayload {
	/// JS that loads the module (only for kernel modules) represented as a UnixFs file
	loader: Vec<HashMap<&'static str, String>>,

	/// WASM payload of the module itself represented as a UnixFs file
	module: Vec<HashMap<&'static str, String>>,
}

/// Represents metadata attached to a DAO.
#[derive(Serialize, Debug)]
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
	let entries: Vec<Cid> = future::try_join_all(modules.into_iter().enumerate().map(
		async move |(i, (mut load, mut module))| {
			// Modules have a WASM and JS payload. Load the WASM
			let mut src = Vec::new();
			module.read_to_end(&mut src)?;

			// And load the JavaScript
			let mut loader = Vec::new();
			load.read_to_end(&mut loader)?;

			let loader_cid = ipfs.add(Cursor::new(src)).await.map_err(Error::Ipfs)?.hash;
			let module_cid = ipfs
				.add(Cursor::new(loader))
				.await
				.map_err(Error::Ipfs)?
				.hash;

			let loader_cid_rep = {
				let mut m = HashMap::new();
				m.insert("/", loader_cid);

				m
			};
			let mod_cid_rep = {
				let mut m = HashMap::new();
				m.insert("/", module_cid);

				m
			};

			let module = IdeaPayload {
				loader: vec![loader_cid_rep],
				module: vec![mod_cid_rep],
			};

			// Upload the metadata to IPFS
			ipfs.dag_put(Cursor::new(serde_json::to_string(&module)?))
				.map_ok(|resp| {
					log::debug!("finished deploying module {}", i);

					resp.cid
				})
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
