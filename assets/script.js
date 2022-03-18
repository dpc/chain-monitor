function showReconnecting() {
  document.getElementById('reconnecting').style.display = 'block';
}

function showConnLost() {
  document.getElementById('reconnecting').style.display = 'none';
  document.getElementById('conn-lost').style.display = 'block';
}

function showConnected() {
  document.getElementById('conn-lost').style.display = 'none';
  document.getElementById('reconnecting').style.display = 'none';
}


class ChainsState {
  constructor(sources, sourcesFullName, chains, chainsFullName) {
    this.sources = sources;
    this.sourcesFullName = sourcesFullName;
    this.chains = chains;
    this.chainsFullName = chainsFullName;
    this.states = [];
    this.bestHeight = Array(chains.length).fill(0);
  }

  update(source, chain, chainState) {
    console.log(`update: ${source} ${chain} ${this.getIdxByIds(source, chain)} ${JSON.stringify(chainState)}`);
    this.states[this.getIdxByIds(source, chain)] = chainState;

    const bestHeightIdx = this.getChainIdx(chain);
    if (this.bestHeight[bestHeightIdx] < chainState.height) {
      this.bestHeight[bestHeightIdx] = chainState.height;
    }
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

  getChainIdx(chain) {
    return this.chains.findIndex((element) => element === chain);
  }

  renderTable() {
    const table = document.createElement('table');

    console.log(`Rendering: ${JSON.stringify(this)}`);

    {
      const headerTr = document.createElement('tr');

      {
        const th = document.createElement('th');
        th.appendChild(document.createTextNode(''));
        headerTr.appendChild(th);
      }

      {
        const th = document.createElement('th');
        th.appendChild(document.createTextNode('Best'));
        headerTr.appendChild(th);
      }

      for (var sourceIdx = 0; sourceIdx < this.sources.length; sourceIdx++){
        const th = document.createElement('th');
        const div = document.createElement('div');
        div.setAttribute('scope', 'col');
        div.appendChild(document.createTextNode(this.sources[sourceIdx]));
        div.classList.add('tooltip');

        const span = document.createElement('span');
        span.appendChild(document.createTextNode(this.sourcesFullName[sourceIdx]));
        span.classList.add('tooltiptext');

        div.appendChild(span);
        th.appendChild(div);

        headerTr.appendChild(th);
      }

      table.appendChild(headerTr);
    }


    for (var chainIdx = 0; chainIdx < this.chains.length; chainIdx++) {
      const bestHeight = this.bestHeight[chainIdx];

      const tr = document.createElement('tr');
      table.appendChild(tr);

      {
        const th = document.createElement('td');
        th.setAttribute('scope', 'row');
        th.appendChild(document.createTextNode(this.chainsFullName[chainIdx]));
        tr.appendChild(th);
      }

      {
        const td = document.createElement('td');
        td.appendChild(document.createTextNode(bestHeight));
        tr.appendChild(td);
      }

      for (var sourceIdx = 0; sourceIdx < this.sources.length; sourceIdx++){
        const stateIdx = this.getIdx(sourceIdx, chainIdx);
        const td = document.createElement('td');
        tr.appendChild(td);
        const div = document.createElement('div');
        td.appendChild(div);
        div.classList.add('tooltip');

        const chainState = this.states[stateIdx];
        if (chainState) {
          const span = document.createElement('span');
          div.appendChild(span);
          span.appendChild(document.createTextNode(`height: ${chainState.height}`));
          span.appendChild(document.createElement('br'));
          span.appendChild(document.createTextNode(`hash:`));
          span.innerHTML += '&nbsp;';
          span.appendChild(document.createTextNode(`${chainState.hash}`));
          span.appendChild(document.createElement('br'));
          span.appendChild(document.createTextNode(`ts: ${new Date(1000 * chainState.ts).toISOString()}`));
          span.classList.add('tooltiptext');

          const diff = chainState.height - bestHeight;
          if (diff >= -1) {
            td.classList.add('at-chainhead');
          } else {
            td.classList.add('not-at-chainhead');
          }
          div.appendChild(document.createTextNode(diff));
        } else {
          div.appendChild(document.createTextNode(""));
            td.classList.add('missing-state');
        }
      }
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
    });

    socket.addEventListener('message', function (event) {
      console.log('Message from server ', event.data);
      const msg = JSON.parse(event.data);

      if (msg.type === 'init') {
        app.chains = new ChainsState(msg.sources, msg.sourcesFullName, msg.chains, msg.chainsFullName);
        app.renderChains();
      } else if (msg.type === 'update') {
        app.chains.update(msg.source, msg.chain, msg);
        app.renderChains();
      }
    });

    socket.addEventListener('error', function (err) {
      console.log('Socket encountered error: ', err);
      socket.close();
    });


    socket.addEventListener('close', function () {
      showConnLost();
      console.log('Socket is closed. Reconnecting soon...');
      setTimeout(function() {
        app.connect();
      }, 10000 + Math.random() * 10000);
    });
  }
}

window.onload = function() {
  console.log("Page loaded.");

  const app = new App();
  app.connect();
};
