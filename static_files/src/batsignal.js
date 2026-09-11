;(function ($w, $d, H, L) {
  'use strict';
  // $w: window, $d: document, H: history, L: location URL, S: private node state
  const S = new WeakMap(), N = new Set()
  S.set($w, {ac: new AbortController(), req: new Set()})

  /**
   * DOM node selector utility. Will use querySelectorAll() if a callback
   * is provided, otherwise querySelector().
   * @param {Element} parent - Parent element to search within.
   * @param {string} selector - CSS selector string.
   * @param {Function} [cb] - Callback for each matched element (Optional)
   * @returns {Element|Array<*>} - Single element if no callback, array of
   *   callback results otherwise.
   */
  const $ = ($p, s, cb) => !cb ? (s ? $p : $d).querySelector(s ?? $p) : Array.from($p.querySelectorAll(s), cb)
  if (!$w.$) $w.$ = $

  /**
   * Compiles a string into an executable function.
   * The compiled function receives: el (target node), evt (event object),
   * and ctx (batsignal helpers bound to el).
   *
   * @param {Node} el - The target DOM node.
   * @param {string} body - JavaScript code string to compile.
   * @returns {Function} A function that takes an event and executes the compiled code.
   */
  function compile(el, body) {
    try {
      const cb = new Function('el', 'evt', 'ctx', body), signal = S.get(el)?.ac.signal
      return (evt) => cb(el, evt, {
        fetch: (...a) => fetch(el, ...a),
        listen: (el, name, cb, opt = {}) => listen(el, name, cb, {signal, ...opt}),
        dispatch,
        signal,
      })
    } catch (error) {
      console.error(error, el, body)
    }
  }

  /**
   * Dispatches a custom event on a given node.
   * @param {Node} el - The target DOM node
   * @param {string} name - The event name (emitted as 'sse-{eventName}').
   * @param {Object} [detail={}] - Detail for CustomEvent
   * @param {Object} [opt={}] - Additional CustomEvent options (e.g. bubbles).
   * @returns {void}
   */
  const dispatch = (el, name, detail = {}, opt = {}) =>
    el.dispatchEvent(new CustomEvent(name, {bubbles: false, ...opt, detail}))

  /**
   * Listen
   * @param {Node} el - The target DOM node
   * @param {string} name - The event name
   * @param {Function} cb - Callback for event listener
   * @param {Object} [opt={}] - Additional options (e.g. once).
   * @returns {void}
   */
  const listen = (el, name, cb, opt = {}) =>
    el.addEventListener(name, cb, opt.signal ? opt : {...opt, signal: S.get(el).ac.signal})

  /**
   * Fetches a resource and dispatches appropriate events based on content type.
   * Headers can be injected via: <meta name="fetch-headers" content='"X-Foo": "bar"'>
   * Requests run concurrently unless the `navigation` option aborts a pending navigation request.
   *
   * @param {Node} el - The target DOM node (for cleanup tracking).
   * @param {string} url - A relative or absolute URL to fetch.
   * @param {Object} [opt={}] - Fetch options (method, headers, body, search, navigation, etc).
   *   The 'signal' option is managed internally and will be overridden.
   * @returns {Promise<Response|null>} - The fetch Response, or null on error.
   *
   * @fires sse-patch-elements - Dispatched when content-type is text/html
   * @fires sse-message - Dispatched when content-type contains "json"
   * @fires sse-{event} - Dispatched for each SSE event when content-type is text/event-stream
   * @fires sse-unknown - Dispatched for unrecognized content types
   * @fires fetch - Dispatched when starting and ending a request
   */
  async function fetch(el, url, opt = {}) {
    const state = S.get(el) || S.get($w), ac = new AbortController()

    try {
      const u = new URL(url.replace(/\#.*/, ''), L.href)
      if (opt.search)
        for (const k in opt.search)
          u.searchParams.append(k, JSON.stringify(opt.search[k]).replace(/^"|"$/g, ''))

      const $h = $($d.head, 'meta[name=fetch-headers]')
      const headers = new Headers(opt.headers)
      for (const [name, value] of Object.entries($h ? compile($h, `return {${$h.content}}`)() : {}))
        headers.append(name, value)
      if (opt.isSSE && opt.lastEventId) headers.set('Last-Event-ID', opt.lastEventId)

      dispatch(el, 'fetch', {options: opt, headers, url: u}, {bubbles: true})
      if (opt.navigation) for (const ac of N) ac.abort()
      state.req.add(ac)
      if (opt.navigation) N.add(ac)
      const r = await $w.fetch(u, {...opt, headers, signal: ac.signal})
      dispatch(el, 'fetch', {response: r}, {bubbles: true})

      const ct = r.headers.get('content-type') ?? ''
      if (ct.startsWith('text/html')) {
        dispatch(el, 'sse-patch-elements', {data: await r.text(), url}, {bubbles: true})
      } else if (ct.match(/\bjson\b/)) {
        dispatch(el, 'sse-message', {data: await r.text(), url}, {bubbles: true})
      } else if (ct.startsWith('text/event-stream')) {
        opt.isSSE = true
        const decoder = new TextDecoder('utf-8'), reader = r.body.getReader()
        let buf = '', sse = {}
        for (;;) {
          const {done, value} = await reader.read()
          if (done) break
          buf += decoder.decode(value, {stream: true})
          for (let i; (i = buf.indexOf('\n')) >= 0;) {
            const line = buf.slice(0, i).replace(/\r$/, '')
            if (!line) {
              if (sse.data != undefined) dispatch(el, 'sse-' + (sse.event ?? 'message'), {data: sse.data.slice(0, -1), url}, {bubbles: true})
              sse = {}
            } else if (!line.startsWith(':')) {
              const colon = line.indexOf(':')
              const [k, v] = colon < 0 ? [line, ''] : [line.slice(0, colon), line.slice(colon + 1).replace(/^ /, '')]
              if (k == 'data') sse.data = (sse.data ?? '') + v + '\n'
              else if (k == 'id' && !v.includes('\0')) opt.lastEventId = v
              else if (k == 'retry' && /^\d+$/.test(v)) opt.retry = Number(v)
              else {
                sse[k] ??= ''
                sse[k] += v
              }
            }
            buf = buf.slice(i + 1)
          }
        }
        return reconnect(new Error('SSE stream closed'))
      } else {
        dispatch(el, 'sse-unknown', {response: r, url}, {bubbles: true})
      }

      return r
    } catch (error) {
      if (ac.signal.aborted) {
        return null
      } else if (opt.isSSE) {
        return reconnect(error)
      } else {
        dispatch(el, 'fetch', {error, options: opt, url}, {bubbles: true})
        return null
      }
    } finally {
      state.req.delete(ac)
      N.delete(ac)
    }

    async function reconnect(error) {
      const delay = opt.retry ?? 3000
      console.debug(`batsignal reconnects SSE ${url} after ${delay}ms because ${error}`)
      await new Promise(resolve => setTimeout(resolve, delay))
      return !ac.signal.aborted && el.parentNode ? fetch(el, url, opt) : null
    }
  }

  /*
   * Initializes event listeners on all elements with batsignal attributes.
   * Called automatically on page load and after patching with sse-patch-elements.
   *
   * Supported attributes:
   *   on:load       - Executes code once when element is initialized
   *   on:click      - Executes code on click event
   *   on:input      - Executes code on input events
   *   on:value      - Runs for input/change events and external 'value' events
   *   on:{event}    - Executes code on any DOM event
   *
   * Each element is initialized only once (tracked in private WeakMap state).
   */
  function init() {
    $($d, '[on\\:load]', (el) => {
      if (S.has(el)) return
      S.set(el, {ac: new AbortController(), req: new Set()})

      const load = [];
      for (const a of el.attributes) {
        const match = a.name.match(/^on:(.+)/)
        if (!match) continue;

        const event = match[1].split('|')
        const opt = event.slice(1).reduce((opt, n) => { opt[n] = true; return opt }, {})
        const cb = compile(el, a.value)
        if (event[0] == 'load') {
          load.unshift(cb)
        } else if (event[0] == 'value') {
          if (el.tagName == 'SELECT' || el.type == 'checkbox' || el.type == 'radio') {
            listen(el, 'change', cb, opt)
          } else if (el.tagName == 'INPUT' || el.tagName == 'TEXTAREA') {
            listen(el, 'input', cb, opt)
          }

          listen(el, 'value', ({detail}) => {
            if (detail != undefined) el.value = detail
            cb()
          }, opt)

          load.push(cb)
        } else {
          listen(el, event[0], cb, opt)
        }
      }

      load.forEach(cb => cb())
    })
    dispatch($d, 'ready')
  }

  // Parses HTML responses and swaps elements in the DOM based on data-swap attributes.
  listen($w, 'sse-patch-elements', ({detail: {data, url}}) => {
    if (!data) return

    const hasBody = data.indexOf('<body') != -1
    const dom = hasBody ? new DOMParser().parseFromString(data, 'text/html') : (() => {
      const t = $d.createElement('template')
      t.innerHTML = data
      return t.content
    })()

    $(dom, '[data-swap=ignore]', el => el.remove())
    $(dom, '[data-swap=keep]', b => {
      const a = b.id && $d.getElementById(b.id)
      a ? b.replaceWith(a) : b.remove()
    })

    const [script, style] = ['script', 'style'].map(sel => $(dom, sel, el => [el, el.remove()][0]))
    hasBody
      ? $(dom, 'title', t => $($d, 'title').textContent = t.textContent)
      : Array.from(dom.children).forEach(el => el.id && !el.dataset.swap && (el.dataset.swap = 'morph'))
    $($d, '[data-owner]', el => (hasBody || (url && el.dataset.owner == url)) && el.remove())
    style.forEach(el => $d.head.appendChild([el, (el.dataset.owner = url || '')][0]))
    hasBody ? (swap(dom) || swapBody(dom)) : swap(dom)
    script.forEach(el => {
      const copy = $d.createElement('script')
      for (const {name, value} of el.attributes) copy.setAttribute(name, value)
      copy.dataset.owner = url || ''
      copy.textContent = el.textContent
      $d.head.appendChild(copy)
    })
    init()

    function destroy(el) {
      dispatch(el, 'destroy')
      $(el, '[on\\:load]', destroy)
      const value = S.get(el)
      value?.ac.abort()
      for (const ac of value?.req.values() ?? []) ac.abort()
      S.delete(el)
    }

    function swap(parsed) {
      return $(parsed, '[data-swap]', b => {
        const [m, sel] = b.dataset.swap.split(':', 2)
        if (m == 'keep') return false
        const a = sel ? $($d, sel) : $d.getElementById(b.id)
        if (!a || (m != 'morph' && typeof a[m] != 'function')) return console.warn('batsignal can\'t swap', {m, sel, a, b}), true
        if (m == 'remove') return destroy(a), a.remove(), true
        if (m == 'morph' || m == 'replaceWith') destroy(a)
        m == 'morph' ? Idiomorph.morph(a, b) : a[m](b)
        return true
      }).filter(r => r).length > 0
    }

    function swapBody(parsed) {
      const [a, b] = [$d.body, $(parsed, 'body')]
      destroy(a)
      for (const {name} of [...a.attributes]) if (!b.hasAttribute(name)) a.removeAttribute(name)
      for (const {name, value} of b.attributes) a.setAttribute(name, value)
      a.replaceChildren(...b.childNodes)
      if (L.hash) $($d, L.hash, el => el.scrollIntoView({behavior: 'auto'}))
    }
  })

  // Listens for click events on links and intercepts them for SPA navigation.
  listen($w, 'click', (evt) => {
    const el = evt.target?.closest('a[href], area[href]')
    if (!el || evt.defaultPrevented || evt.button != 0 || evt.metaKey || evt.ctrlKey || evt.shiftKey || evt.altKey || el.target || el.hasAttribute('download')) return

    const url = new URL(el.href || el.getAttribute('href'), L.href)
    if (url.origin != L.origin) return // Not the same site
    if (url.pathname == L.pathname && url.search == L.search && url.hash) return // link#anchor on same page

    const m = el.dataset.history || 'pushState'
    if (m != 'none') H[m]({}, null, url.pathname + url.search + url.hash)

    evt.preventDefault()
    fetch(el, url.pathname + url.search, {method: 'GET', navigation: true})
  })

  // Listens for popstate events (back/forward navigation) and fetches the new page content.
  listen($w, 'popstate', () => {
    const O = L
    L = new URL(location.href)
    if (O.pathname == L.pathname && O.search == L.search) return
    fetch($d.body, L.pathname + L.search, {navigation: true})
  })

  // Listens for form submissions and intercepts them for SPA navigation.
  listen($w, 'submit', (evt) => {
    const el = evt.target?.closest('form')
    const $s = evt.submitter
    if (!el || evt.defaultPrevented || el.target || $s?.formTarget) return

    const u = new URL($s?.hasAttribute('formaction') ? $s.formAction : el.action || L.href)
    if (u.origin != L.origin) return // Not the same site

    const method = $s?.formMethod || el.method
    if (method.toLowerCase() == 'dialog') return

    const [b, r] = [new FormData(el, $s), {method, navigation: true}]

    const m = el.dataset.history || 'pushState'
    if (r.method.toLowerCase() == 'post') {
      const c = 'application/x-www-form-urlencoded'
      const t = $s?.formEnctype || el.enctype || c
      r.body = t == c ? new URLSearchParams(b) : b
    } else {
      for (const [k, v] of b.entries()) u.searchParams.append(k, v)
    }

    if (m != 'none') H[m]({}, null, u.toString())
    if ($s) $s.ariaBusy = 'true'
    evt.preventDefault()
    fetch(el, u.toString(), r).finally(() => {
      if ($s) $s.ariaBusy = 'false'
    })
  })

  // Monkey-patches history.pushState and history.replaceState so that `L` stays in sync regardless of which code calls them
  ;['pushState', 'replaceState'].forEach(m => {
    const o = H[m].bind(H)
    H[m] = (s, t, u) => { o(s, t, u); u && (L = new URL(u, L.href)) }
  })

  init()
})(window, document, history, new URL(location.href))
