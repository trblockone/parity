// Copyright 2015, 2016 Ethcore (UK) Ltd.
// This file is part of Parity.

// Parity is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity.  If not, see <http://www.gnu.org/licenses/>.

//! Parity-specific rpc implementation.
use std::sync::{Arc, Weak};
use std::str::FromStr;
use std::collections::BTreeMap;

use util::{RotatingLogger, Address};
use util::misc::version_data;

use crypto::ecies;
use ethkey::{Brain, Generator};
use ethstore::random_phrase;
use ethsync::{SyncProvider, ManageNetwork};
use ethcore::miner::MinerService;
use ethcore::client::{MiningBlockChainClient};
use ethcore::ids::BlockID;
use ethcore::mode::Mode;
use ethcore::account_provider::AccountProvider;

use jsonrpc_core::Error;
use v1::traits::Parity;
use v1::types::{
	Bytes, U256, H160, H256, H512,
	Peers, Transaction, RpcSettings, Histogram,
	TransactionStats, LocalTransactionStatus,
};
use v1::helpers::{errors, SigningQueue, SignerService, NetworkSettings};
use v1::helpers::dispatch::DEFAULT_MAC;

/// Parity implementation.
pub struct ParityClient<C, M, S: ?Sized> where
	C: MiningBlockChainClient,
	M: MinerService,
	S: SyncProvider,
{
	client: Weak<C>,
	miner: Weak<M>,
	sync: Weak<S>,
	net: Weak<ManageNetwork>,
	accounts: Weak<AccountProvider>,
	logger: Arc<RotatingLogger>,
	settings: Arc<NetworkSettings>,
	signer: Option<Arc<SignerService>>,
	dapps_interface: Option<String>,
	dapps_port: Option<u16>,
}

impl<C, M, S: ?Sized> ParityClient<C, M, S> where
	C: MiningBlockChainClient,
	M: MinerService,
	S: SyncProvider,
{
	/// Creates new `ParityClient`.
	pub fn new(
		client: &Arc<C>,
		miner: &Arc<M>,
		sync: &Arc<S>,
		net: &Arc<ManageNetwork>,
		store: &Arc<AccountProvider>,
		logger: Arc<RotatingLogger>,
		settings: Arc<NetworkSettings>,
		signer: Option<Arc<SignerService>>,
		dapps_interface: Option<String>,
		dapps_port: Option<u16>,
	) -> Self {
		ParityClient {
			client: Arc::downgrade(client),
			miner: Arc::downgrade(miner),
			sync: Arc::downgrade(sync),
			net: Arc::downgrade(net),
			accounts: Arc::downgrade(store),
			logger: logger,
			settings: settings,
			signer: signer,
			dapps_interface: dapps_interface,
			dapps_port: dapps_port,
		}
	}

	fn active(&self) -> Result<(), Error> {
		// TODO: only call every 30s at most.
		take_weak!(self.client).keep_alive();
		Ok(())
	}
}

impl<C, M, S: ?Sized> Parity for ParityClient<C, M, S> where
	M: MinerService + 'static,
	C: MiningBlockChainClient + 'static,
	S: SyncProvider + 'static {

	fn transactions_limit(&self) -> Result<usize, Error> {
		try!(self.active());

		Ok(take_weak!(self.miner).transactions_limit())
	}

	fn min_gas_price(&self) -> Result<U256, Error> {
		try!(self.active());

		Ok(U256::from(take_weak!(self.miner).minimal_gas_price()))
	}

	fn extra_data(&self) -> Result<Bytes, Error> {
		try!(self.active());

		Ok(Bytes::new(take_weak!(self.miner).extra_data()))
	}

	fn gas_floor_target(&self) -> Result<U256, Error> {
		try!(self.active());

		Ok(U256::from(take_weak!(self.miner).gas_floor_target()))
	}

	fn gas_ceil_target(&self) -> Result<U256, Error> {
		try!(self.active());

		Ok(U256::from(take_weak!(self.miner).gas_ceil_target()))
	}

	fn dev_logs(&self) -> Result<Vec<String>, Error> {
		try!(self.active());

		let logs = self.logger.logs();
		Ok(logs.as_slice().to_owned())
	}

	fn dev_logs_levels(&self) -> Result<String, Error> {
		try!(self.active());

		Ok(self.logger.levels().to_owned())
	}

	fn net_chain(&self) -> Result<String, Error> {
		try!(self.active());

		Ok(self.settings.chain.clone())
	}

	fn net_peers(&self) -> Result<Peers, Error> {
		try!(self.active());

		let sync = take_weak!(self.sync);
		let sync_status = sync.status();
		let net_config = take_weak!(self.net).network_config();
		let peers = sync.peers().into_iter().map(Into::into).collect();

		Ok(Peers {
			active: sync_status.num_active_peers,
			connected: sync_status.num_peers,
			max: sync_status.current_max_peers(net_config.min_peers, net_config.max_peers),
			peers: peers
		})
	}

	fn net_port(&self) -> Result<u16, Error> {
		try!(self.active());

		Ok(self.settings.network_port)
	}

	fn node_name(&self) -> Result<String, Error> {
		try!(self.active());

		Ok(self.settings.name.clone())
	}

	fn registry_address(&self) -> Result<Option<H160>, Error> {
		try!(self.active());

		Ok(
			take_weak!(self.client)
				.additional_params()
				.get("registrar")
				.and_then(|s| Address::from_str(s).ok())
				.map(|s| H160::from(s))
		)
	}

	fn rpc_settings(&self) -> Result<RpcSettings, Error> {
		try!(self.active());
		Ok(RpcSettings {
			enabled: self.settings.rpc_enabled,
			interface: self.settings.rpc_interface.clone(),
			port: self.settings.rpc_port as u64,
		})
	}

	fn default_extra_data(&self) -> Result<Bytes, Error> {
		try!(self.active());

		Ok(Bytes::new(version_data()))
	}

	fn gas_price_histogram(&self) -> Result<Histogram, Error> {
		try!(self.active());
		take_weak!(self.client).gas_price_histogram(100, 10).ok_or_else(errors::not_enough_data).map(Into::into)
	}

	fn unsigned_transactions_count(&self) -> Result<usize, Error> {
		try!(self.active());

		match self.signer {
			None => Err(errors::signer_disabled()),
			Some(ref signer) => Ok(signer.len()),
		}
	}

	fn generate_secret_phrase(&self) -> Result<String, Error> {
		try!(self.active());

		Ok(random_phrase(12))
	}

	fn phrase_to_address(&self, phrase: String) -> Result<H160, Error> {
		try!(self.active());

		Ok(Brain::new(phrase).generate().unwrap().address().into())
	}

	fn list_accounts(&self) -> Result<Option<Vec<H160>>, Error> {
		try!(self.active());

		Ok(take_weak!(self.client)
			.list_accounts(BlockID::Latest)
			.map(|a| a.into_iter().map(Into::into).collect()))
	}

	fn list_storage_keys(&self, _address: H160) -> Result<Option<Vec<H256>>, Error> {
		try!(self.active());

		// TODO: implement this
		Ok(None)
	}

	fn encrypt_message(&self, key: H512, phrase: Bytes) -> Result<Bytes, Error> {
		try!(self.active());

		ecies::encrypt(&key.into(), &DEFAULT_MAC, &phrase.0)
			.map_err(errors::encryption_error)
			.map(Into::into)
	}

	fn pending_transactions(&self) -> Result<Vec<Transaction>, Error> {
		try!(self.active());

		Ok(take_weak!(self.miner).all_transactions().into_iter().map(Into::into).collect::<Vec<_>>())
	}

	fn pending_transactions_stats(&self) -> Result<BTreeMap<H256, TransactionStats>, Error> {
		try!(self.active());

		let stats = take_weak!(self.sync).transactions_stats();
		Ok(stats.into_iter()
		   .map(|(hash, stats)| (hash.into(), stats.into()))
		   .collect()
		)
	}

	fn local_transactions(&self) -> Result<BTreeMap<H256, LocalTransactionStatus>, Error> {
		try!(self.active());

		let transactions = take_weak!(self.miner).local_transactions();
		Ok(transactions
		   .into_iter()
		   .map(|(hash, status)| (hash.into(), status.into()))
		   .collect()
		)
	}

	fn signer_port(&self) -> Result<u16, Error> {
		try!(self.active());

		self.signer
			.clone()
			.and_then(|signer| signer.address())
			.map(|address| address.1)
			.ok_or_else(|| errors::signer_disabled())
	}

	fn dapps_port(&self) -> Result<u16, Error> {
		try!(self.active());

		self.dapps_port
			.ok_or_else(|| errors::dapps_disabled())
	}

	fn dapps_interface(&self) -> Result<String, Error> {
		try!(self.active());

		self.dapps_interface.clone()
			.ok_or_else(|| errors::dapps_disabled())
	}

	fn next_nonce(&self, address: H160) -> Result<U256, Error> {
		try!(self.active());
		let address: Address = address.into();
		let miner = take_weak!(self.miner);
		let client = take_weak!(self.client);

		Ok(miner.last_nonce(&address)
			.map(|n| n + 1.into())
			.unwrap_or_else(|| client.latest_nonce(&address))
			.into()
		)
	}

	fn mode(&self) -> Result<String, Error> {
		Ok(match take_weak!(self.client).mode() {
			Mode::Off => "offline",
			Mode::Dark(..) => "dark",
			Mode::Passive(..) => "passive",
			Mode::Active => "active",
		}.into())
	}

	fn enode(&self) -> Result<String, Error> {
		take_weak!(self.sync).enode().ok_or_else(errors::network_disabled)
	}

	fn accounts(&self) -> Result<BTreeMap<String, BTreeMap<String, String>>, Error> {
		try!(self.active());
		let store = take_weak!(self.accounts);
		let info = try!(store.accounts_info().map_err(|e| errors::account("Could not fetch account info.", e)));
		let other = store.addresses_info().expect("addresses_info always returns Ok; qed");

		Ok(info.into_iter().chain(other.into_iter()).map(|(a, v)| {
			let mut m = map![
				"name".to_owned() => v.name,
				"meta".to_owned() => v.meta
			];
			if let &Some(ref uuid) = &v.uuid {
				m.insert("uuid".to_owned(), format!("{}", uuid));
			}
			(format!("0x{}", a.hex()), m)
		}).collect())
	}
}
