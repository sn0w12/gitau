// Injected before page scripts by the plugin (Tauri wraps this in an IIFE).
'use strict'

var internals = window.__TAURI_INTERNALS__
if (!internals || typeof internals.invoke !== 'function' || typeof navigator === 'undefined') {
  return
}

var COMMAND = 'plugin:window-controls-overlay|'
var EVENT = 'window-controls-overlay://geometrychange'

var CUSTOM_PROPERTIES = [
  '--titlebar-area-x',
  '--titlebar-area-y',
  '--titlebar-area-width',
  '--titlebar-area-height',
  '--window-controls-x',
  '--window-controls-y',
  '--window-controls-width',
  '--window-controls-height',
  '--leading-window-controls-x',
  '--leading-window-controls-y',
  '--leading-window-controls-width',
  '--leading-window-controls-height'
]

function invoke(command, args) {
  return internals.invoke(COMMAND + command, args)
}

function emptyGeometry() {
  var rect = function () {
    return { x: 0, y: 0, width: 0, height: 0 }
  }
  return {
    visible: false,
    titlebarArea: rect(),
    windowControls: rect(),
    leadingWindowControls: rect()
  }
}

// Work queued until the document has an <html> element. The init script runs
// before the document is parsed, so `documentElement` can still be null.
var pending = []

// The bootstrap is fast enough to publish geometry before the queued initial
// defaults run at DOMContentLoaded. Once geometry has been published, those
// defaults must not overwrite it.
var published = false

function withRoot(fn) {
  if (typeof document === 'undefined') {
    return
  }
  var root = document.documentElement
  if (root) {
    fn(root)
  } else {
    pending.push(fn)
  }
}

// Substituted with `std::env::consts::OS` when the plugin is built, matching the
// values Tauri's `platform()` returns.
var PLATFORM = '__PLATFORM__'

withRoot(function (root) {
  root.dataset.platform = PLATFORM
  if (!published) {
    root.dataset.windowControlsSide = 'none'
  }
})

if (typeof document !== 'undefined' && document.readyState === 'loading') {
  document.addEventListener(
    'DOMContentLoaded',
    function () {
      var root = document.documentElement
      if (!root) {
        return
      }
      var queued = pending
      pending = []
      for (var i = 0; i < queued.length; i++) {
        queued[i](root)
      }
    },
    { once: true }
  )
}

/**
 * Physical side of the native buttons. A right-to-left Windows window moves its
 * trailing group to `x = 0`, and Linux can have a group on either side.
 */
function controlsSide(geometry) {
  var leading = geometry.leadingWindowControls.width > 0
  var controls = geometry.windowControls
  if (leading && controls.width > 0) {
    return 'both'
  }
  if (leading) {
    return 'left'
  }
  if (controls.width <= 0) {
    return 'none'
  }
  return controls.x < geometry.titlebarArea.x ? 'left' : 'right'
}

/**
 * Mirrors the geometry onto `:root`. While the overlay is hidden the geometry
 * properties are removed rather than zeroed, so `var()` fallbacks apply.
 */
function publish(geometry) {
  published = true
  withRoot(function (root) {
    root.dataset.windowControlsOverlay = geometry.visible ? 'visible' : 'hidden'
    root.dataset.windowControlsSide = controlsSide(geometry)

    if (!geometry.visible) {
      for (var i = 0; i < CUSTOM_PROPERTIES.length; i++) {
        root.style.removeProperty(CUSTOM_PROPERTIES[i])
      }
      return
    }

    var titlebarArea = geometry.titlebarArea
    var windowControls = geometry.windowControls
    var leading = geometry.leadingWindowControls
    var values = {
      '--titlebar-area-x': titlebarArea.x,
      '--titlebar-area-y': titlebarArea.y,
      '--titlebar-area-width': titlebarArea.width,
      '--titlebar-area-height': titlebarArea.height,
      '--window-controls-x': windowControls.x,
      '--window-controls-y': windowControls.y,
      '--window-controls-width': windowControls.width,
      '--window-controls-height': windowControls.height,
      '--leading-window-controls-x': leading.x,
      '--leading-window-controls-y': leading.y,
      '--leading-window-controls-width': leading.width,
      '--leading-window-controls-height': leading.height
    }
    for (var j = 0; j < CUSTOM_PROPERTIES.length; j++) {
      var property = CUSTOM_PROPERTIES[j]
      root.style.setProperty(property, values[property] + 'px')
    }
  })
}

function publishColors(state) {
  withRoot(function (root) {
    if (!state || !state.enabled) {
      root.style.removeProperty('--titlebar-color')
      root.style.removeProperty('--titlebar-symbol-color')
      return
    }
    root.style.setProperty('--titlebar-color', state.color)
    root.style.setProperty('--titlebar-symbol-color', state.symbolColor)
  })
}

/**
 * Recomputes the horizontal geometry from the viewport width. The plugin's
 * `geometrychange` travels through IPC and can arrive a frame late; `resize`
 * fires before style recalculation.
 */
function reflow(geometry) {
  if (!geometry.visible || typeof window === 'undefined') {
    return geometry
  }
  var titlebar = geometry.titlebarArea
  var controls = geometry.windowControls
  var leading = geometry.leadingWindowControls
  var free = Math.max(0, window.innerWidth - controls.width - leading.width)
  var titlebarX = leading.width > 0 ? leading.width : titlebar.x
  var controlsTrailing = controls.width > 0 && (leading.width > 0 || titlebar.x === 0)
  return {
    visible: geometry.visible,
    titlebarArea: {
      x: titlebarX,
      y: titlebar.y,
      width: free,
      height: titlebar.height
    },
    windowControls: {
      x: controlsTrailing ? titlebarX + free : 0,
      y: controls.y,
      width: controls.width,
      height: controls.height
    },
    leadingWindowControls: {
      x: leading.x,
      y: leading.y,
      width: leading.width,
      height: leading.height
    }
  }
}

function sameRect(a, b) {
  return a.x === b.x && a.y === b.y && a.width === b.width && a.height === b.height
}

function sameGeometry(a, b) {
  return (
    a.visible === b.visible &&
    sameRect(a.titlebarArea, b.titlebarArea) &&
    sameRect(a.windowControls, b.windowControls) &&
    sameRect(a.leadingWindowControls, b.leadingWindowControls)
  )
}

function toDOMRect(rect) {
  return new DOMRect(rect.x, rect.y, rect.width, rect.height)
}

/**
 * A `navigator.windowControlsOverlay`-compatible object. Chromium only defines
 * it for installed PWAs; the extra `enable` / `set` / `disable` / `state`
 * methods are Tauri extensions.
 *
 * `env(titlebar-area-*)` cannot be polyfilled: WebView2 and WebKitGTK do not
 * implement it, and `env()` is not settable from JavaScript.
 */
class WindowControlsOverlay extends EventTarget {
  #geometry = emptyGeometry()

  /**
   * Resolves with the initial state once the plugin has auto-enabled the
   * overlay, or with `null` when it is not enabled on this window. Available
   * before any page script runs.
   */
  ready

  get visible() {
    return this.#geometry.visible
  }

  getTitlebarAreaRect() {
    return toDOMRect(this.#geometry.titlebarArea)
  }

  /** Tauri extension. */
  getWindowControlsRect() {
    return toDOMRect(this.#geometry.windowControls)
  }

  /**
   * Tauri extension: the buttons a Linux desktop puts on the leading side.
   */
  getLeadingWindowControlsRect() {
    return toDOMRect(this.#geometry.leadingWindowControls)
  }

  /**
   * Enables the overlay, or reconfigures it if it is already on.
   *
   * Omit `options` to use the plugin configuration. Pass `true` for the same,
   * or `{ color, symbolColor, height }` to override individual values.
   * Resolves with the new state.
   */
  enable(options) {
    var args = { options: options === undefined ? true : options }
    return invoke('enable', args).then((state) => {
      this._applyState(state)
      return state
    })
  }

  /**
   * Updates the colors and height of an enabled overlay. Unspecified fields
   * keep their current value.
   */
  set(options) {
    return invoke('set', { options: options }).then((state) => {
      this._applyState(state)
      return state
    })
  }

  disable() {
    return invoke('disable').then(() => {
      this._applyState(null)
    })
  }

  state() {
    return invoke('get_state')
  }

  /** @internal */
  _apply(geometry) {
    var next = reflow(geometry)
    if (sameGeometry(next, this.#geometry)) {
      return
    }
    this.#geometry = next
    publish(next)
    this.dispatchEvent(
      new CustomEvent('geometrychange', {
        detail: {
          boundingRect: next.titlebarArea,
          titlebarAreaRect: next.titlebarArea,
          visible: next.visible,
          windowControlsRect: next.windowControls,
          leadingWindowControlsRect: next.leadingWindowControls
        }
      })
    )
  }

  /** @internal */
  _applyState(state) {
    publishColors(state)
    if (!state || !state.enabled) {
      this._apply(emptyGeometry())
      return
    }
    this._apply({
      visible: state.visible,
      titlebarArea: state.titlebarArea,
      windowControls: state.windowControls,
      leadingWindowControls: state.leadingWindowControls
    })
  }

  /** @internal */
  _refresh() {
    this._apply(this.#geometry)
  }
}

var overlay = new WindowControlsOverlay()
Object.defineProperty(navigator, 'windowControlsOverlay', {
  value: overlay,
  writable: false,
  configurable: true
})

// Runs before the plugin's IPC event for the same resize, so the CSS variables
// never trail the native edge.
if (typeof window !== 'undefined') {
  window.addEventListener('resize', function () {
    overlay._refresh()
  })
}

var label =
  internals.metadata && internals.metadata.currentWindow
    ? internals.metadata.currentWindow.label
    : null

overlay.ready = internals
  .invoke('plugin:event|listen', {
    event: EVENT,
    target: { kind: 'Window', label: label },
    handler: internals.transformCallback(function (event) {
      overlay._apply(event.payload)
    })
  })
  // Listening before enabling means the enable event cannot be missed.
  .then(function () {
    return invoke('bootstrap')
  })
  .then(function (state) {
    overlay._applyState(state)
    return state
  })
  .catch(function (error) {
    console.warn('[window-controls-overlay]', error)
    return null
  })
