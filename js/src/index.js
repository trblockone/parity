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

import 'babel-polyfill';
import 'whatwg-fetch';

import es6Promise from 'es6-promise';
es6Promise.polyfill();

import React from 'react';
import ReactDOM from 'react-dom';
import injectTapEventPlugin from 'react-tap-event-plugin';
import { createHashHistory } from 'history';
import { Redirect, Router, Route, useRouterHistory } from 'react-router';
import qs from 'querystring';

import SecureApi from './secureApi';
import ContractInstances from './contracts';
import { initLocales } from './i18n';

initLocales('en');

import { initStore } from './redux';
import { ContextProvider, muiTheme } from './ui';
import { Accounts, Account, Addresses, Address, Application, Contract, Contracts, WriteContract, Dapp, Dapps, Settings, SettingsBackground, SettingsParity, SettingsProxy, SettingsViews, Signer, Status } from './views';

import { setApi } from './redux/providers/apiActions';

import './environment';

import '../assets/fonts/Roboto/font.css';
import '../assets/fonts/RobotoMono/font.css';

import styles from './reset.css';
import './index.html';

injectTapEventPlugin();

if (process.env.NODE_ENV === 'development') {
  // Expose the React Performance Tools on the`window` object
  const Perf = require('react-addons-perf');
  window.Perf = Perf;
}

const AUTH_HASH = '#/auth?';
const parityUrl = process.env.PARITY_URL ||
  (
    process.env.NODE_ENV === 'production'
    ? window.location.host
    : '127.0.0.1:8180'
  );

let token = null;
if (window.location.hash && window.location.hash.indexOf(AUTH_HASH) === 0) {
  token = qs.parse(window.location.hash.substr(AUTH_HASH.length)).token;
}

const api = new SecureApi(`ws://${parityUrl}`, token);
ContractInstances.create(api);

const store = initStore(api);
store.dispatch({ type: 'initAll', api });
store.dispatch(setApi(api));

window.secureApi = api;

const routerHistory = useRouterHistory(createHashHistory)({});

ReactDOM.render(
  <ContextProvider api={ api } muiTheme={ muiTheme } store={ store }>
    <Router className={ styles.reset } history={ routerHistory }>
      <Redirect from='/' to='/accounts' />
      <Redirect from='/auth' to='/accounts' query={ {} } />
      <Redirect from='/settings' to='/settings/views' />
      <Route path='/' component={ Application }>
        <Route path='accounts' component={ Accounts } />
        <Route path='account/:address' component={ Account } />
        <Route path='addresses' component={ Addresses } />
        <Route path='address/:address' component={ Address } />
        <Route path='apps' component={ Dapps } />
        <Route path='app/:id' component={ Dapp } />
        <Route path='contracts' component={ Contracts } />
        <Route path='contracts/write' component={ WriteContract } />
        <Route path='contract/:address' component={ Contract } />
        <Route path='settings' component={ Settings }>
          <Route path='background' component={ SettingsBackground } />
          <Route path='proxy' component={ SettingsProxy } />
          <Route path='views' component={ SettingsViews } />
          <Route path='parity' component={ SettingsParity } />
        </Route>
        <Route path='signer' component={ Signer } />
        <Route path='status' component={ Status } />
        <Route path='status/:subpage' component={ Status } />
      </Route>
    </Router>
  </ContextProvider>,
  document.querySelector('#container')
);
