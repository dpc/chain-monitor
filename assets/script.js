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


function playSound() {
  try {
    new Audio( 'sound1.mp3').play();
  } catch (e) {
    console.log("Unable to play sound: ${e}");
  }
}

class ChainsState {
  constructor(sources, chains) {
    this.sources = sources;
    this.chains = chains;
    this.states = [];
    this.bestHeight = Array(chains.length).fill(0);
    // max time between the source updates that backend can guarantee
    this.MAX_BACKEND_SOURCE_CHECK_PERIOD_SECS = 60;
    this.loadEnableSoundFor();
  }

  dbgEnableSoundFor() {
    console.log(`enableSoundFor: ${JSON.stringify(this.enableSoundFor)}`);
  }

  loadEnableSoundFor() {
    this.enableSoundFor = JSON.parse(window.localStorage.getItem('enableSoundFor') || '{}');
    this.dbgEnableSoundFor();
  }

  saveEnableSoundFor() {
    window.localStorage.setItem('enableSoundFor', JSON.stringify(this.enableSoundFor));
    this.dbgEnableSoundFor();
  }

  enableSoundForIdx(source, chain) {
    return source + '###' + chain;
  }

  toggleEnableSoundFor(source, chain) {
    if (this.getEnableSoundFor(source, chain)) {
      delete this.enableSoundFor[this.enableSoundForIdx(source, chain)];
    } else {
      this.enableSoundFor[this.enableSoundForIdx(source, chain)] = true;
    }
  }

  getEnableSoundFor(source, chain) {
    return this.enableSoundFor[this.enableSoundForIdx(source, chain)] || false;
  }

  update(source, chain, chainState) {
    const stateIdx = this.getIdxByIds(source, chain);
    const prevState = this.states[stateIdx];
    this.states[stateIdx] = chainState;

    const bestHeightIdx = this.getChainIdx(chain);
    if (this.bestHeight[bestHeightIdx] < chainState.height) {
      this.bestHeight[bestHeightIdx] = chainState.height;
    }

    if ((prevState === undefined || prevState.hash != chainState.height) && this.getEnableSoundFor(source, chain)) {
      playSound();
    }
  }

  getIdxByIds(source, chain) {
    return this.getIdx(
      this.sources.findIndex((element) => element.id === source),
      this.chains.findIndex((element) => element.id === chain)
    );
  }

  getIdx(sourceIdx, chainIdx) {
    return sourceIdx * this.chains.length + chainIdx;
  }

  getChainIdx(chain) {
    return this.chains.findIndex((element) => element.id === chain);
  }

  addSoundToggleToElement (element, source, chain) {
    const this_ = this;
    element.addEventListener('click', function(e) {
      this_.toggleEnableSoundFor(source, chain);
      this_.saveEnableSoundFor();
      window.app.redraw();
    });
  }

  addStopOnClickPropagationToElement (element) {
    const this_ = this;
    element.addEventListener('click', function(e) {
      e.stopPropagation();
    });
  }

  createSoundToggleButtonElement (source, chain) {
    const toggleSound = document.createElement('i');
    toggleSound.href = '#';
    toggleSound.classList.add(`sound-${this.getEnableSoundFor(source, chain) ? 'enabled' : 'disabled' }`)
    const this_ = this;
    toggleSound.addEventListener('click', function(e) {
      this_.toggleEnableSoundFor(source, chain);
      this_.saveEnableSoundFor();
      window.app.redraw();
    });
    return toggleSound;
  }
  renderTable() {
    const table = document.createElement('table');

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
        div.appendChild(document.createTextNode(this.sources[sourceIdx].shortName));
        div.classList.add('tooltip');

        const span = document.createElement('span');
        span.appendChild(document.createTextNode(this.sources[sourceIdx].fullName));
        span.classList.add('tooltiptext');

        div.appendChild(span);
        th.appendChild(div);

        headerTr.appendChild(th);
      }

      table.appendChild(headerTr);
    }


    for (var chainIdx = 0; chainIdx < this.chains.length; chainIdx++) {
      const chain = this.chains[chainIdx];
      const bestHeight = this.bestHeight[chainIdx];

      const tr = document.createElement('tr');
      table.appendChild(tr);

      {
        const th = document.createElement('td');
        th.setAttribute('scope', 'row');
        th.appendChild(document.createTextNode(this.chains[chainIdx].fullName));
        tr.appendChild(th);
      }

      {
        const td = document.createElement('td');
        td.appendChild(document.createTextNode(bestHeight));
        tr.appendChild(td);
      }

      for (var sourceIdx = 0; sourceIdx < this.sources.length; sourceIdx++){
        const source = this.sources[sourceIdx];
        const stateIdx = this.getIdx(sourceIdx, chainIdx);
        const chainState = this.states[stateIdx];

        const td = document.createElement('td');
        tr.appendChild(td);

        if (chainState) {
          td.classList.add(`sound-${this.getEnableSoundFor(source.id, chain.id) ? 'enabled' : 'disabled' }`)
          this.addSoundToggleToElement(td, source.id, chain.id);
        }
        const div = document.createElement('div');
        td.appendChild(div);
        div.classList.add('tooltip');

        if (chainState) {
          const diff = chainState.height - bestHeight;
          const nowTs = new Date().getTime() / 1000;
          const stalenessSecs = Math.round(nowTs - chainState.firstSeenTs);

          const span = document.createElement('span');
          const secsAgo = chainState.firstSeenTs
          div.appendChild(span);
          this.addStopOnClickPropagationToElement(span);
          span.appendChild(document.createTextNode(`height: ${chainState.height}`));
          span.appendChild(document.createElement('br'));
          span.appendChild(document.createTextNode(`hash:`));
          span.innerHTML += '&nbsp;';
          span.appendChild(document.createTextNode(`${chainState.hash}`));
          span.appendChild(document.createElement('br'));
          span.appendChild(document.createTextNode(`first seen: ${new Date(1000 * chainState.firstSeenTs).toISOString()} (${stalenessSecs}s ago)`));
          span.classList.add('tooltiptext');

          if (diff >= -1) {
            td.classList.add('at-chainhead');
          } else {
            td.classList.add('not-at-chainhead');
          }

          if (stalenessSecs < 25) {
            td.classList.add('just-increased');
          }
          if (stalenessSecs > Math.max(chain.blockTimeSecs * 3, this.MAX_BACKEND_SOURCE_CHECK_PERIOD_SECS)) {
            td.classList.add('stale');
          }

          const diffSpan = document.createElement('span');
          div.appendChild(diffSpan);
          diffSpan.appendChild(document.createTextNode(diff));
          diffSpan.classList.add('height');
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
    this.reconnectCount = 0;

  }

  redraw() {
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
      app.reconnectCount = 0;

      if (msg.type === 'init') {
        app.chains = new ChainsState(msg.sources, msg.chains);
        app.redraw();
      } else if (msg.type === 'update') {
        app.chains.update(msg.source, msg.chain, msg);
        app.redraw();
      }
    });

    socket.addEventListener('error', function (err) {
      console.log('Socket encountered error: ', err);
      socket.close();
    });


    socket.addEventListener('close', function () {
      showConnLost();
      const reconnectDelay = Math.min(60000, 1000 * app.reconnectCount * (Math.random() + 0.5))
      console.log(`Socket is closed. Reconnecting soon (${reconnectDelay}ms...`);
      app.reconnectCount++;
      setTimeout(function() {
        app.connect();
      }, reconnectDelay);
    });
  }
}

window.onload = function() {
  console.log("Page loaded.");

  const app = new App();
  window.app = app;
  app.connect();
};
