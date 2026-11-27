/**
 * @name VinciSkin Bridge
 * @description Reports the champ-select pick to the VinciSkin manager.
 */

const ENDPOINT = 'http://127.0.0.1:__PORT__/state?token=__TOKEN__'
const EVENT = 'OnJsonApiEvent_lol-champ-select_v1_session'
const PHASE_EVENT = 'OnJsonApiEvent_lol-gameflow_v1_phase'

let retry = 1000
let last = ''

/** Send the pick when it changes, never on a clock. */
function post(championId, skinId) {
  const body = JSON.stringify({ championId, skinId })
  if (body === last) return

  last = body
  fetch(ENDPOINT, { method: 'POST', mode: 'no-cors', body }).catch(() => {})
}

function report(eventType, session) {
  if (eventType === 'Delete' || !session) {
    post(0, 0)
    return
  }

  const me = (session.myTeam || []).find(player => player.cellId === session.localPlayerCellId)
  post(me ? Number(me.championId) || 0 : 0, me ? Number(me.selectedSkinId) || 0 : 0)
}

/**
 * Follow the session over the client's own event stream.
 *
 * The page is served by the local API, so the socket rides the credentials the
 * page already has. Nothing here runs on a timer: a tick per second is what
 * made the previous bridge leak a handle per request.
 */
function connect() {
  const socket = new WebSocket(`wss://${location.host}/`)

  socket.onopen = () => {
    retry = 1000
    socket.send(JSON.stringify([5, EVENT]))
    socket.send(JSON.stringify([5, PHASE_EVENT]))

    // Events only arrive on a change, so read once to seed the current pick.
    fetch('/lol-champ-select/v1/session')
      .then(response => (response.ok ? response.json() : response.text().then(() => null)))
      .then(session => report('Update', session))
      .catch(() => {})
  }

  socket.onmessage = event => {
    let message

    try {
      message = JSON.parse(event.data)
    } catch {
      return
    }

    if (!Array.isArray(message) || message[0] !== 8) return

    if (message[1] === PHASE_EVENT) {
      const phase = (message[2] || {}).data
      if (phase === 'GameStart' || phase === 'InProgress' || phase === 'Reconnect') {
        post(0, 0)
      }
      return
    }

    if (message[1] !== EVENT) return

    const payload = message[2] || {}
    report(payload.eventType, payload.data)
  }

  socket.onclose = () => {
    setTimeout(connect, retry)
    retry = Math.min(retry * 2, 30000)
  }
}

connect()
