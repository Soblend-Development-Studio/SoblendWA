(() => {
  'use strict'

  const pages = [...document.querySelectorAll('.page')]
  const pagesById = new Map(pages.map((page) => [page.id, page]))
  const navLinks = [...document.querySelectorAll('.nav-link')]
  const main = document.querySelector('.main')
  const toc = document.querySelector('.toc')
  const pager = document.querySelector('.pager')
  const sidebar = document.querySelector('.sidebar')
  const scrim = document.querySelector('.sidebar-scrim')
  const homeId = pages[0].id
  const baseTitle = 'SoblendWA'

  function remember(key, value) {
    try {
      localStorage.setItem(key, value)
    } catch {
      return
    }
  }

  function recall(key) {
    try {
      return localStorage.getItem(key)
    } catch {
      return null
    }
  }

  function fold(text) {
    return text.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase()
  }

  function setTheme(theme) {
    document.documentElement.dataset.theme = theme
    remember('soblendwa-theme', theme)
    document.querySelectorAll('.theme-toggle').forEach((button) => {
      const next = theme === 'dark' ? 'claro' : 'oscuro'
      button.setAttribute('aria-label', `Cambiar a tema ${next}`)
      button.setAttribute('title', `Cambiar a tema ${next}`)
    })
  }

  function currentTheme() {
    const chosen = document.documentElement.dataset.theme
    if (chosen === 'dark' || chosen === 'light') return chosen
    return matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
  }

  document.querySelectorAll('.theme-toggle').forEach((button) => {
    button.addEventListener('click', () => setTheme(currentTheme() === 'dark' ? 'light' : 'dark'))
  })

  function highlightPage(page) {
    if (page.dataset.highlighted || !window.hljs) return
    page.dataset.highlighted = 'true'
    page.querySelectorAll('pre code[class*="language-"]').forEach((block) => {
      if (!block.classList.contains('language-text')) window.hljs.highlightElement(block)
    })
  }

  const copyIcon =
    '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="9" y="9" width="12" height="12" rx="2"></rect><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path></svg>'

  document.querySelectorAll('.code-block').forEach((block) => {
    const button = document.createElement('button')
    button.type = 'button'
    button.className = 'copy-button'
    button.innerHTML = copyIcon
    button.setAttribute('aria-label', 'Copiar código')
    button.addEventListener('click', async () => {
      const code = block.querySelector('code')
      try {
        await navigator.clipboard.writeText(code.textContent)
        button.textContent = 'Copiado'
      } catch {
        const range = document.createRange()
        range.selectNodeContents(code)
        const selection = getSelection()
        selection.removeAllRanges()
        selection.addRange(range)
        button.textContent = 'Seleccionado'
      }
      setTimeout(() => {
        button.innerHTML = copyIcon
      }, 1500)
    })
    block.appendChild(button)
  })

  function selectLanguage(group, language) {
    const buttons = [...group.querySelectorAll(':scope > .tab-list > .tab-button')]
    const target = buttons.find((button) => button.dataset.lang === language)
    if (!target) return false
    buttons.forEach((button) => {
      const selected = button === target
      button.setAttribute('aria-selected', String(selected))
      button.tabIndex = selected ? 0 : -1
      group.querySelector(`#${button.getAttribute('aria-controls')}`).hidden = !selected
    })
    return true
  }

  function applyLanguage(language) {
    document.querySelectorAll('.tabs').forEach((group) => selectLanguage(group, language))
  }

  document.querySelectorAll('.tabs').forEach((group) => {
    const buttons = [...group.querySelectorAll(':scope > .tab-list > .tab-button')]
    buttons.forEach((button, index) => {
      button.addEventListener('click', () => {
        const before = button.getBoundingClientRect().top
        applyLanguage(button.dataset.lang)
        remember('soblendwa-language', button.dataset.lang)
        scrollBy(0, button.getBoundingClientRect().top - before)
      })
      button.addEventListener('keydown', (event) => {
        const step = event.key === 'ArrowRight' ? 1 : event.key === 'ArrowLeft' ? -1 : 0
        if (!step) return
        event.preventDefault()
        const next = buttons[(index + step + buttons.length) % buttons.length]
        next.focus()
        next.click()
      })
    })
  })

  const savedLanguage = recall('soblendwa-language')
  if (savedLanguage) applyLanguage(savedLanguage)

  function buildToc(page) {
    const headings = [...page.querySelectorAll('h2[id], h3[id]')]
    toc.replaceChildren()
    if (headings.length < 2) return
    const title = document.createElement('p')
    title.className = 'toc-title'
    title.textContent = 'En esta página'
    const list = document.createElement('ul')
    for (const heading of headings) {
      const item = document.createElement('li')
      const link = document.createElement('a')
      link.href = `#${heading.id}`
      link.textContent = heading.firstChild.textContent.trim()
      if (heading.tagName === 'H3') link.classList.add('is-sub')
      item.appendChild(link)
      list.appendChild(item)
    }
    toc.append(title, list)
  }

  let observer = null

  function watchHeadings(page) {
    if (observer) observer.disconnect()
    const links = new Map([...toc.querySelectorAll('a')].map((link) => [link.getAttribute('href').slice(1), link]))
    if (!links.size || !('IntersectionObserver' in window)) return
    const mark = (id) => {
      links.forEach((link, key) => link.classList.toggle('is-current', key === id))
    }
    observer = new IntersectionObserver(
      (entries) => {
        const visible = entries.filter((entry) => entry.isIntersecting)
        if (visible.length) mark(visible[0].target.id)
      },
      { rootMargin: '-10% 0px -75% 0px' },
    )
    page.querySelectorAll('h2[id], h3[id]').forEach((heading) => observer.observe(heading))
  }

  function buildPager(page) {
    const index = pages.indexOf(page)
    const previous = pages[index - 1]
    const next = pages[index + 1]
    pager.replaceChildren()
    const link = (target, label, className) => {
      const anchor = document.createElement('a')
      anchor.href = `#${target.id}`
      anchor.className = className
      const small = document.createElement('small')
      small.textContent = label
      anchor.append(small, target.dataset.title)
      return anchor
    }
    if (previous) pager.appendChild(link(previous, 'Anterior', 'pager-previous'))
    if (next) pager.appendChild(link(next, 'Siguiente', 'pager-next'))
  }

  function closeSidebar() {
    sidebar.classList.remove('is-open')
    scrim.classList.remove('is-open')
  }

  let activePage = null

  function show(page, target) {
    if (activePage !== page) {
      pages.forEach((item) => item.classList.toggle('is-active', item === page))
      navLinks.forEach((link) => {
        if (link.getAttribute('href') === `#${page.id}`) link.setAttribute('aria-current', 'page')
        else link.removeAttribute('aria-current')
      })
      main.classList.toggle('is-wide', page.dataset.wide === 'true')
      highlightPage(page)
      buildToc(page)
      watchHeadings(page)
      buildPager(page)
      document.title = page.id === homeId ? `Documentación de ${baseTitle}` : `${page.dataset.title} · ${baseTitle}`
      activePage = page
    }
    closeSidebar()
    document.querySelectorAll('.is-target').forEach((item) => item.classList.remove('is-target'))
    if (target) {
      target.classList.add('is-target')
      target.scrollIntoView()
    } else {
      scrollTo(0, 0)
    }
  }

  function route() {
    const id = decodeURIComponent(location.hash.slice(1))
    if (pagesById.has(id)) return show(pagesById.get(id), null)
    const target = id ? document.getElementById(id) : null
    const owner = target ? target.closest('.page') : null
    if (owner) return show(owner, target)
    return show(pagesById.get(homeId), null)
  }

  addEventListener('hashchange', route)

  document.querySelectorAll('.menu-button').forEach((button) => {
    button.addEventListener('click', () => {
      sidebar.classList.add('is-open')
      scrim.classList.add('is-open')
    })
  })
  scrim.addEventListener('click', closeSidebar)

  const layer = document.querySelector('.search-layer')
  const input = document.querySelector('#search-input')
  const results = document.querySelector('.search-results')
  let searchIndex = null
  let selected = 0
  let matches = []

  function buildIndex() {
    const entries = []
    for (const page of pages) {
      const where = page.dataset.group ? `${page.dataset.group} · ${page.dataset.title}` : page.dataset.title
      entries.push({ title: page.dataset.title, where: page.dataset.group || 'Documentación', href: page.id, rank: 0, body: '' })
      page.querySelectorAll('h2[id], h3[id]').forEach((heading) => {
        entries.push({ title: heading.firstChild.textContent.trim(), where, href: heading.id, rank: 1, body: '' })
      })
      page.querySelectorAll('.api[id]').forEach((api) => {
        entries.push({ title: api.dataset.name, where, href: api.id, rank: 2, body: api.textContent })
      })
      let heading = null
      page.querySelectorAll('h2[id], h3[id], p, li, td').forEach((node) => {
        if (node.matches('h2, h3')) {
          heading = node
          return
        }
        if (node.closest('.api')) return
        const href = heading ? heading.id : page.id
        entries.push({ title: null, text: node.textContent, where, href, rank: 3 })
        if (node.matches('td')) {
          node.querySelectorAll('code').forEach((code) => {
            entries.push({ title: code.textContent, where, href, rank: 2, body: '' })
          })
        }
      })
    }
    for (const entry of entries) {
      entry.key = fold(entry.title || '')
      entry.short = entry.key.split(/\.|::/).pop()
      entry.haystack = fold(entry.text || entry.body || '')
    }
    return entries
  }

  function scoreEntry(entry, query) {
    let score = -1
    if (entry.key === query || entry.short === query) score = 120
    else if (entry.key.startsWith(query) || entry.short.startsWith(query)) score = 100
    else if (entry.key.includes(query)) score = 80
    else if (entry.text && entry.haystack.includes(query)) score = 40
    else if (entry.haystack.includes(query)) score = 30
    if (score < 0) return score
    return score - entry.rank + (entry.where.startsWith('Referencia') ? 5 : 0)
  }

  function snippet(text, query) {
    const folded = fold(text)
    const at = folded.indexOf(query)
    if (at < 0) return text.slice(0, 90)
    const start = Math.max(0, at - 34)
    return `${start > 0 ? '…' : ''}${text.slice(start, at + query.length + 56).trim()}…`
  }

  function highlightMatch(container, text, query) {
    const at = fold(text).indexOf(query)
    if (at < 0 || !query) {
      container.textContent = text
      return
    }
    const mark = document.createElement('mark')
    mark.textContent = text.slice(at, at + query.length)
    container.append(text.slice(0, at), mark, text.slice(at + query.length))
  }

  function search(raw) {
    const query = fold(raw.trim())
    if (!searchIndex) searchIndex = buildIndex()
    if (!query) return searchIndex.filter((entry) => entry.rank === 0)
    const scored = []
    const seen = new Set()
    for (const entry of searchIndex) {
      const score = scoreEntry(entry, query)
      if (score < 0) continue
      const dedupe = entry.title ? `${entry.href}|${entry.title}` : `${entry.href}|text`
      if (seen.has(dedupe)) continue
      seen.add(dedupe)
      scored.push({ entry, score })
    }
    scored.sort((a, b) => b.score - a.score)
    return scored.slice(0, 40).map((item) => item.entry)
  }

  function renderResults() {
    const query = fold(input.value.trim())
    results.replaceChildren()
    if (!matches.length) {
      const empty = document.createElement('li')
      empty.className = 'search-empty'
      empty.textContent = `Sin resultados para "${input.value.trim()}". Prueba con el nombre de un método, un evento o un tema.`
      results.appendChild(empty)
      return
    }
    matches.forEach((entry, index) => {
      const item = document.createElement('li')
      const link = document.createElement('a')
      link.className = `search-result${index === selected ? ' is-selected' : ''}`
      link.href = `#${entry.href}`
      link.id = `search-result-${index}`
      link.setAttribute('role', 'option')
      link.setAttribute('aria-selected', String(index === selected))
      const title = document.createElement('span')
      title.className = 'search-result-title'
      highlightMatch(title, entry.title || snippet(entry.text, query), query)
      const where = document.createElement('span')
      where.className = 'search-result-where'
      where.textContent = entry.where
      link.append(title, where)
      link.addEventListener('click', closeSearch)
      item.appendChild(link)
      results.appendChild(item)
    })
    const current = results.querySelector('.is-selected')
    if (current) current.scrollIntoView({ block: 'nearest' })
  }

  function updateSearch() {
    matches = search(input.value)
    selected = 0
    renderResults()
  }

  function openSearch() {
    layer.classList.add('is-open')
    input.value = ''
    updateSearch()
    input.focus()
  }

  function closeSearch() {
    layer.classList.remove('is-open')
  }

  document.querySelectorAll('.search-trigger').forEach((button) => button.addEventListener('click', openSearch))
  input.addEventListener('input', updateSearch)
  layer.addEventListener('click', (event) => {
    if (event.target === layer) closeSearch()
  })

  input.addEventListener('keydown', (event) => {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault()
      if (!matches.length) return
      const step = event.key === 'ArrowDown' ? 1 : -1
      selected = (selected + step + matches.length) % matches.length
      renderResults()
    } else if (event.key === 'Enter') {
      event.preventDefault()
      const entry = matches[selected]
      if (entry) {
        closeSearch()
        if (location.hash === `#${entry.href}`) route()
        else location.hash = entry.href
      }
    }
  })

  addEventListener('keydown', (event) => {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') {
      event.preventDefault()
      if (layer.classList.contains('is-open')) closeSearch()
      else openSearch()
      return
    }
    if (event.key === 'Escape') {
      closeSearch()
      closeSidebar()
      return
    }
    const typing = /^(INPUT|TEXTAREA|SELECT)$/.test(event.target.tagName)
    if (event.key === '/' && !typing && !event.ctrlKey && !event.metaKey) {
      event.preventDefault()
      openSearch()
    }
  })

  if (/Mac|iPhone|iPad/.test(navigator.platform)) {
    document.querySelectorAll('.search-trigger kbd').forEach((kbd) => {
      kbd.textContent = '⌘K'
    })
  }

  setTheme(currentTheme())
  route()
  if (window.hljs) highlightPage(activePage)
  else addEventListener('load', () => highlightPage(activePage))
})()
