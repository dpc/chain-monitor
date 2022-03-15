function showReconnecting() {
  document.getElementById('reconnecting').style.display = 'block';
}

function showConnError(error) {
  document.getElementById('conn-error').textContent = error + ' ';
}

function showConnected() {
  document.getElementById('reconnecting').style.display = 'none';
}


class ChainsState {
  constructor(sources, chains) {
    this.sources = sources;
    this.chains = chains;
    this.states = [];
  }

  update(source, chain, chainState) {
    console.log(`update: ${source} ${chain} ${this.getIdxByIds(source, chain)} ${JSON.stringify(chainState)}`);
    this.states[this.getIdxByIds(source, chain)] = chainState;
  }

  getIdxByIds(source, chain) {
    return this.getIdx(
      this.sources.findIndex((element) => element === source),
      this.chains.findIndex((element) => element === chain)
    );
  }

  getIdx(sourceIdx, chainIdx) {
    return sourceIdx * this.chains.length + chainIdx;
  }

  renderTable() {
    const table = document.createElement('table');
    // table.border ='1';

    console.log(`Rendering: ${JSON.stringify(this)}`);

    {
      const headerTr = document.createElement('tr');

      {
        const th = document.createElement('th');
        th.appendChild(document.createTextNode('Coins \\ Sources'));
        headerTr.appendChild(th);
      }

      for (var sourceIdx = 0; sourceIdx < this.sources.length; sourceIdx++){
        const th = document.createElement('th');
        th.setAttribute('scope', 'col');
        th.appendChild(document.createTextNode(this.sources[sourceIdx]));
        headerTr.appendChild(th);
      }

      table.appendChild(headerTr);
    }


    for (var chainIdx = 0; chainIdx < this.chains.length; chainIdx++) {
      const tr = document.createElement('tr');

      {
        const th = document.createElement('td');
        th.setAttribute('scope', 'row');
        th.appendChild(document.createTextNode(this.chains[chainIdx]));
        tr.appendChild(th);
      }

      for (var sourceIdx = 0; sourceIdx < this.sources.length; sourceIdx++){
        const td = document.createElement('td');
        const chainState = this.states[this.getIdx(sourceIdx, chainIdx)];
        td.appendChild(document.createTextNode(chainState ? chainState.height : ""));
        tr.appendChild(td);
      }

      table.appendChild(tr);
    }

    return table;
  }

}

class App {
  constructor() {

  }

  renderChains() {
    const stateTableContainer = document.getElementById("state-table")
    stateTableContainer.innerHTML = '';
    stateTableContainer.appendChild(this.chains.renderTable());
  }

  connect() {
    console.log('Reconnecting...');
    showReconnecting();

    var url = new URL('/ws', window.location.href);
    url.protocol = url.protocol.replace('http', 'ws');
    const socket = new WebSocket(url);

    const app = this;

    socket.addEventListener('open', function (event) {
      showConnected();
      console.log('Connected.');
      socket.send('Hello Server!');
    });

    socket.addEventListener('message', function (event) {
      console.log('Message from server ', event.data);
      const msg = JSON.parse(event.data);

      if (msg.type === 'init') {
        app.chains = new ChainsState(msg.sources, msg.chains);
        app.renderChains();
      } else if (msg.type === 'update') {
        app.chains.update(msg.source, msg.chain, msg.state);
        app.renderChains();
      }
    });

    socket.addEventListener('error', function (err) {
      showConnError('Connection error.');
      console.log('Socket encountered error: ', err);
      socket.close();
    });


    socket.addEventListener('close', function () {
      console.log('Socket is closed. Reconnecting soon...');
      setTimeout(function() {
        app.connect();
      }, 1000 + Math.random() * 1000);
    });
  }
}

window.onload = function() {
  console.log("Page loaded.");

  const app = new App();
  app.connect();
};
