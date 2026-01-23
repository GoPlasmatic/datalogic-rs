/**
 * DataLogic Playground Loader
 *
 * This script loads the DataLogic embed bundle which includes React and all dependencies.
 * It then initializes the visual debugger widgets and playground.
 *
 * Widgets are rendered using the @goplasmatic/datalogic-ui component.
 */

(function () {
  'use strict';

  // Get the base path for the documentation site
  // This handles both local development and GitHub Pages deployment
  function getBasePath() {
    // Check for defined path prefix (used by mdBook)
    const pathPrefix = document.querySelector('meta[name="path_prefix"]');
    if (pathPrefix) {
      return pathPrefix.getAttribute('content') || '';
    }
    // Fallback: detect from current script location
    const scripts = document.getElementsByTagName('script');
    for (const script of scripts) {
      if (script.src && script.src.includes('datalogic-playground')) {
        const url = new URL(script.src);
        // Extract base path (everything before /theme/)
        const match = url.pathname.match(/^(.*?)\/theme\//);
        if (match) {
          return match[1];
        }
      }
    }
    return '';
  }

  const BASE_PATH = getBasePath();

  // Local bundle paths (in assets folder, copied during build)
  const EMBED_JS = `${BASE_PATH}/assets/datalogic-embed.js`;
  const EMBED_CSS = `${BASE_PATH}/assets/datalogic-embed.css`;

  // State tracking
  let initialized = false;
  let loadError = null;

  /**
   * Load a script from URL
   */
  function loadScript(url) {
    return new Promise((resolve, reject) => {
      // Check if already loaded
      if (document.querySelector(`script[src="${url}"]`)) {
        resolve();
        return;
      }

      const script = document.createElement('script');
      script.src = url;
      script.crossOrigin = 'anonymous';
      script.onload = resolve;
      script.onerror = () => reject(new Error(`Failed to load: ${url}`));
      document.head.appendChild(script);
    });
  }

  /**
   * Load a stylesheet from URL
   */
  function loadStylesheet(url) {
    return new Promise((resolve, reject) => {
      // Check if already loaded
      if (document.querySelector(`link[href="${url}"]`)) {
        resolve();
        return;
      }

      const link = document.createElement('link');
      link.rel = 'stylesheet';
      link.href = url;
      link.crossOrigin = 'anonymous';
      link.onload = resolve;
      link.onerror = () => reject(new Error(`Failed to load: ${url}`));
      document.head.appendChild(link);
    });
  }

  // Selector for all widget/playground containers
  const CONTAINER_SELECTOR = '#datalogic-playground, [data-datalogic-playground], [data-datalogic], [data-logic], .playground-widget';

  /**
   * Show loading state in containers
   */
  function showLoading() {
    const containers = document.querySelectorAll(CONTAINER_SELECTOR);

    containers.forEach((container) => {
      if (!container.classList.contains('datalogic-initialized')) {
        container.innerHTML =
          '<div class="datalogic-loading">Loading visual debugger...</div>';
      }
    });
  }

  /**
   * Show error state in containers
   */
  function showError(message) {
    const containers = document.querySelectorAll(CONTAINER_SELECTOR);

    containers.forEach((container) => {
      if (!container.classList.contains('datalogic-initialized')) {
        container.innerHTML = `<div class="datalogic-error">
          <strong>Failed to load playground</strong><br>
          ${message}<br>
          <small>Please refresh the page or check your network connection.</small>
        </div>`;
      }
    });
  }

  /**
   * Load all dependencies and initialize
   */
  async function loadDependencies() {
    if (initialized) {
      // If already initialized, just render widgets
      if (window.DataLogicEmbed) {
        window.DataLogicEmbed.renderWidgets();
      }
      return;
    }

    showLoading();

    try {
      // Load embed stylesheet
      await loadStylesheet(EMBED_CSS);

      // Load our embed bundle (includes React, ReactDOM, and all dependencies)
      await loadScript(EMBED_JS);

      // Initialize the embed system
      if (window.DataLogicEmbed) {
        window.DataLogicEmbed.init();
        initialized = true;
        console.log('DataLogic visual debugger initialized');
      } else {
        throw new Error('DataLogicEmbed not found after loading bundle');
      }
    } catch (error) {
      console.error('Failed to load DataLogic dependencies:', error);
      loadError = error.message;
      showError(error.message);
    }
  }

  /**
   * Handle page navigation (mdBook's client-side navigation)
   */
  function setupNavigationObserver() {
    const content = document.getElementById('content');
    if (!content) return;

    const observer = new MutationObserver((mutations) => {
      // Check if there are new containers to render
      const hasNewContainers = document.querySelector(
        CONTAINER_SELECTOR.split(', ').map(s => s + ':not(.datalogic-initialized)').join(', ')
      );

      if (hasNewContainers) {
        if (loadError) {
          showError(loadError);
        } else if (window.DataLogicEmbed) {
          window.DataLogicEmbed.renderWidgets();
        }
      }
    });

    observer.observe(content, { childList: true, subtree: true });
  }

  /**
   * Initialize on DOM ready
   */
  function init() {
    // Check if there are any containers to initialize
    const hasContainers = document.querySelector(CONTAINER_SELECTOR);

    if (hasContainers) {
      loadDependencies();
    }

    // Set up observer for page navigation
    setupNavigationObserver();
  }

  // Start initialization
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
