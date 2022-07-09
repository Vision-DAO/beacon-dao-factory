# DAO Factory

This program is a command-line utility invokable via `daowiz` that implements
a Beacon DAO-creation wizard.

# Usage

The wizard's main command, `new` creates a new Beacon DAO on the specified
network, falling back to the Polygon Mumbai testnet by default.

## `daowiz new a.wasm b.wasm ... --eth-rpc-uri --ipfs-rpc-uri --contracts-dir`

Creates a new Vision Beacon DAO using the specified:

* `DEPLOYMENT_KEY` (required) - an environment variable specifying the ethereum
private key to use for deploying the DAO
* `--eth-rpc-uri` (required) - a flag specifying the http url of an EVM-compatible
node that daowiz will deploy the Beaon DAO to
* `--ipfs-rpc-uri` (optional) - a flag specifying the http url of an IPFS node
that daowiz will deploy Beacon DAO metadata to. Uses an in-process IPFS node by
default
* `--contracts-dir` (required) - a flag specifying the path to a directory
containing the built Beacon DAO contracts that will be used for deploying the
Beacon DAO
* modules - enumerated paths to `.wasm` files representing all of the modules
that should be installed into the Beacon DAO. Assumes a `.js` loader exists
alongside the loader

## `daowiz ls --eth-rpc-uri --contracts-dir`

Lists the addresses of Beacon DAO's that are instanes of the indicated Beacon
DAO contract, deployed using the indicated:

* `DEPLOYMENT_KEY` (required) - an environment variable specifying the ethereum
private key of the account who should be scanned for deployed Beacon DAO's
* `--eth-rpc-uri` (required) - a flag specifying the http url of an EVM-compatible
node that daowiz will use to scan for Beacon DAO instances
* `--contracts-dir` (required) - a flag specifying the path to a directory
containing the built Beacon DAO contracts that will be used as predicates for
finding deployed instances of the Beacon DAO
