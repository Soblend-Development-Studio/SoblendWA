'use strict'

const fs = require('node:fs')
const path = require('node:path')
const { spawn } = require('node:child_process')
const { pathToFileURL } = require('node:url')

// Opens docs/index.html in a headless Chromium (Edge or Chrome) and clicks through it.
//   node site/browser_test.cjs <browser> docs/index.html <folder for screenshots>
const [, , edge, siteFile, outDir] = process.argv
const port = 9347
const results = []
const problems = []
const consoleErrors = []

function check(name, ok, detail = '') {
  results.push(`${ok ? 'ok   ' : 'FALLA'} ${name}${detail ? ` (${detail})` : ''}`)
  if (!ok) problems.push(name)
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))

async function waitForBrowser() {
  for (let i = 0; i < 60; i += 1) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: 'PUT' })
      if (response.ok) return response.json()
    } catch {
      await sleep(250)
    }
  }
  throw new Error('el navegador no respondio')
}

class Page {
  constructor(socket) {
    this.socket = socket
    this.id = 0
    this.pending = new Map()
    socket.onmessage = (event) => {
      const message = JSON.parse(event.data)
      if (message.id && this.pending.has(message.id)) {
        const { resolve, reject } = this.pending.get(message.id)
        this.pending.delete(message.id)
        if (message.error) reject(new Error(message.error.message))
        else resolve(message.result)
      } else if (message.method === 'Runtime.exceptionThrown') {
        consoleErrors.push(message.params.exceptionDetails.exception?.description || message.params.exceptionDetails.text)
      } else if (message.method === 'Runtime.consoleAPICalled' && message.params.type === 'error') {
        consoleErrors.push(message.params.args.map((a) => a.value).join(' '))
      } else if (message.method === 'Log.entryAdded' && message.params.entry.level === 'error') {
        consoleErrors.push(`${message.params.entry.text} ${message.params.entry.url || ''}`)
      }
    }
  }

  send(method, params = {}) {
    const id = (this.id += 1)
    this.socket.send(JSON.stringify({ id, method, params }))
    return new Promise((resolve, reject) => this.pending.set(id, { resolve, reject }))
  }

  async eval(expression) {
    const out = await this.send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })
    if (out.exceptionDetails) throw new Error(out.exceptionDetails.exception?.description || out.exceptionDetails.text)
    return out.result.value
  }

  async size(width, height, mobile = false) {
    await this.send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: 1, mobile })
  }

  async shot(name, full = false) {
    const { data } = await this.send('Page.captureScreenshot', { format: 'png', captureBeyondViewport: full })
    fs.writeFileSync(path.join(outDir, `${name}.png`), Buffer.from(data, 'base64'))
  }

  async key(key, modifiers = 0, code = key) {
    await this.send('Input.dispatchKeyEvent', { type: 'keyDown', key, code, modifiers, windowsVirtualKeyCode: key === 'Enter' ? 13 : key === 'ArrowDown' ? 40 : key === 'Escape' ? 27 : key.toUpperCase().charCodeAt(0) })
    await this.send('Input.dispatchKeyEvent', { type: 'keyUp', key, code, modifiers })
  }

  async type(text) {
    await this.send('Input.insertText', { text })
  }
}

async function main() {
  fs.mkdirSync(outDir, { recursive: true })
  const browser = spawn(edge, ['--headless=new', '--disable-gpu', `--remote-debugging-port=${port}`, `--user-data-dir=${path.join(outDir, 'profile')}`, '--no-first-run', '--hide-scrollbars', ...(process.platform === 'linux' ? ['--no-sandbox'] : []), 'about:blank'], { stdio: 'ignore' })
  try {
    const target = await waitForBrowser()
    const socket = new WebSocket(target.webSocketDebuggerUrl)
    await new Promise((resolve) => (socket.onopen = resolve))
    const page = new Page(socket)
    await page.send('Page.enable')
    await page.send('Runtime.enable')
    await page.send('Log.enable')
    await page.size(1440, 900)
    const url = pathToFileURL(path.resolve(siteFile)).href
    await page.send('Page.navigate', { url })
    await sleep(3500)

    const ids = await page.eval(`[...document.querySelectorAll('.page')].map((p) => p.id)`)
    check('paginas cargadas', ids.length >= 40, String(ids.length))
    check('arranca en inicio', (await page.eval(`document.querySelector('.page.is-active').id`)) === 'inicio')
    check('highlight.js cargado', await page.eval(`!!window.hljs`))
    check('fuentes cargadas', await page.eval(`document.fonts.check('16px Newsreader')`))
    await page.shot('01-inicio-claro')

    const broken = await page.eval(`[...document.querySelectorAll('a[href^="#"]')].filter((a) => a.getAttribute('href').length > 1 && !document.getElementById(a.getAttribute('href').slice(1))).map((a) => a.getAttribute('href'))`)
    check('enlaces internos', broken.length === 0, broken.slice(0, 5).join(', '))

    const overflowDesktop = []
    const emptyPages = []
    let unhighlighted = 0
    for (const id of ids) {
      await page.eval(`location.hash = ${JSON.stringify(id)}; new Promise((r) => setTimeout(r, 25))`)
      const state = await page.eval(`({ active: [...document.querySelectorAll('.page.is-active')].map((p) => p.id), wide: document.documentElement.scrollWidth - innerWidth, text: document.querySelector('.page.is-active').textContent.length, plain: [...document.querySelectorAll('.page.is-active pre code[class*="language-"]:not(.language-text)')].filter((c) => !c.classList.contains('hljs')).length, title: document.title })`)
      if (state.active.length !== 1 || state.active[0] !== id) problems.push(`router ${id}`)
      if (state.wide > 0) overflowDesktop.push(`${id}+${state.wide}`)
      if (state.text < 200) emptyPages.push(id)
      unhighlighted += state.plain
    }
    check('sin desborde horizontal en escritorio', overflowDesktop.length === 0, overflowDesktop.join(', '))
    check('paginas con contenido', emptyPages.length === 0, emptyPages.join(', '))
    check('bloques resaltados', unhighlighted === 0, String(unhighlighted))

    await page.eval(`location.hash = 'grupos-invitaciones'; new Promise((r) => setTimeout(r, 60))`)
    check('enlace a un titulo abre su pagina', (await page.eval(`document.querySelector('.page.is-active').id`)) === 'grupos')
    check('el titulo queda a la vista', await page.eval(`(() => { const t = document.getElementById('grupos-invitaciones').getBoundingClientRect().top; return t >= 0 && t < innerHeight; })()`))
    check('toc construido', (await page.eval(`document.querySelectorAll('.toc a').length`)) > 3)

    await page.eval(`location.hash = 'enviar'; new Promise((r) => setTimeout(r, 60))`)
    await page.key('k', 2, 'KeyK')
    await sleep(100)
    check('ctrl k abre el buscador', await page.eval(`document.querySelector('.search-layer').classList.contains('is-open')`))
    check('foco en el buscador', await page.eval(`document.activeElement.id === 'search-input'`))
    await page.type('send_text')
    await sleep(150)
    const first = await page.eval(`document.querySelector('.search-result').textContent`)
    check('busca un metodo', first.includes('send_text'), first.slice(0, 50))
    await page.shot('02-buscador')
    await page.key('Enter', 0, 'Enter')
    await sleep(150)
    check('enter abre el resultado', (await page.eval(`location.hash`)).includes('api-client-send-text'), await page.eval(`location.hash`))
    check('pagina del resultado', (await page.eval(`document.querySelector('.page.is-active').id`)) === 'ref-client')
    check('resultado marcado', await page.eval(`document.getElementById('api-client-send-text').classList.contains('is-target')`))
    check('buscador cerrado', !(await page.eval(`document.querySelector('.search-layer').classList.contains('is-open')`)))

    const searchCount = async (text) => {
      await page.eval(`document.querySelector('.search-trigger').click()`)
      await page.eval(`(() => { const i = document.getElementById('search-input'); i.value = ${JSON.stringify(text)}; i.dispatchEvent(new Event('input')); })()`)
      const count = await page.eval(`document.querySelectorAll('.search-result').length`)
      const top = await page.eval(`document.querySelector('.search-result')?.textContent || ''`)
      await page.key('Escape', 0, 'Escape')
      return { count, top }
    }
    const accents = await searchCount('vinculacion')
    check('busca sin acentos', accents.count > 0, accents.top.slice(0, 50))
    const event = await searchCount('pollVote')
    check('busca eventos', event.count > 0, event.top.slice(0, 50))
    const nothing = await searchCount('zzzzqqq')
    check('sin resultados', nothing.count === 0)

    await page.eval(`location.hash = 'enviar'; new Promise((r) => setTimeout(r, 60))`)
    await page.eval(`document.querySelector('.page.is-active .tab-button[data-lang="python"]').click()`)
    await sleep(60)
    const tabState = await page.eval(`(() => { const groups = [...document.querySelectorAll('.tabs')].filter((g) => g.querySelector('.tab-button[data-lang="python"]')); const selected = groups.filter((g) => g.querySelector('.tab-button[aria-selected="true"]').dataset.lang === 'python').length; const visible = groups.every((g) => g.querySelectorAll('.tab-panel:not([hidden])').length === 1); return { groups: groups.length, selected, visible, saved: localStorage.getItem('soblendwa-language') }; })()`)
    check('las pestanas cambian en todo el sitio', tabState.groups === tabState.selected && tabState.visible, JSON.stringify(tabState))
    await page.shot('03-guia-python')
    await page.eval(`document.querySelector('.page.is-active .tab-button[data-lang="rust"]').click()`)

    await page.eval(`document.querySelector('.page.is-active .tab-panel:not([hidden]) .copy-button').click()`)
    await sleep(120)
    const copied = await page.eval(`document.querySelector('.page.is-active .tab-panel:not([hidden]) .copy-button').textContent`)
    check('boton de copiar responde', copied === 'Copiado' || copied === 'Seleccionado', copied)

    const before = await page.eval(`getComputedStyle(document.body).backgroundColor`)
    await page.eval(`document.querySelector('.sidebar .theme-toggle').click()`)
    await sleep(60)
    const after = await page.eval(`getComputedStyle(document.body).backgroundColor`)
    check('cambia el tema', before !== after, `${before} a ${after}`)
    check('tema guardado', (await page.eval(`localStorage.getItem('soblendwa-theme')`)) === 'dark')
    await page.eval(`location.hash = 'inicio'; new Promise((r) => setTimeout(r, 80))`)
    await page.shot('04-inicio-oscuro')
    await page.eval(`location.hash = 'inicio-rapido'; new Promise((r) => setTimeout(r, 80))`)
    await page.shot('05-inicio-rapido-oscuro')
    await page.eval(`location.hash = 'ref-client'; new Promise((r) => setTimeout(r, 80))`)
    await page.shot('06-referencia-oscuro')
    await page.eval(`document.querySelector('.sidebar .theme-toggle').click()`)
    await page.eval(`location.hash = 'rendimiento'; new Promise((r) => setTimeout(r, 80))`)
    await page.shot('07-rendimiento-claro')
    await page.eval(`location.hash = 'confiabilidad'; new Promise((r) => setTimeout(r, 80))`)
    await page.shot('08-confiabilidad-claro')

    await page.size(390, 844, true)
    await sleep(150)
    const overflowMobile = []
    for (const id of ids) {
      await page.eval(`location.hash = ${JSON.stringify(id)}; new Promise((r) => setTimeout(r, 25))`)
      const wide = await page.eval(`document.documentElement.scrollWidth - innerWidth`)
      if (wide > 0) overflowMobile.push(`${id}+${wide}`)
    }
    check('sin desborde horizontal en movil', overflowMobile.length === 0, overflowMobile.join(', '))
    await page.eval(`location.hash = 'inicio'; new Promise((r) => setTimeout(r, 80))`)
    await page.shot('09-movil-inicio')
    await page.eval(`document.querySelector('.menu-button').click()`)
    await sleep(300)
    check('el menu se abre en movil', await page.eval(`document.querySelector('.sidebar').classList.contains('is-open')`))
    await page.shot('10-movil-menu')
    await page.eval(`document.querySelector('.nav-link[href="#grupos"]').click()`)
    await sleep(300)
    check('el menu se cierra al navegar', !(await page.eval(`document.querySelector('.sidebar').classList.contains('is-open')`)))
    await page.shot('11-movil-grupos')

    check('sin errores en la consola', consoleErrors.length === 0, consoleErrors.slice(0, 3).join(' | '))
    for (const line of results) console.log(line)
    console.log(`problemas: ${problems.length}`)
    if (problems.length) console.log(problems.join('\n'))
    socket.close()
  } finally {
    browser.kill()
  }
}

main().then(() => process.exit(problems.length ? 1 : 0), (err) => {
  console.error(err)
  for (const line of results) console.log(line)
  process.exit(1)
})
