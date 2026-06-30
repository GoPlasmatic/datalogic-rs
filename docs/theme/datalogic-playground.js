/**
 * DataLogic Playground Loader
 *
 * This script loads the DataLogic embed bundle which includes React and all dependencies.
 * It then initializes the visual editor widgets and playground.
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
      // The embed bundle is built as an ES module (so wasm-bindgen's
      // `new URL('…', import.meta.url)` resolves against a valid module URL).
      // It must be loaded with type="module"; it sets window.DataLogicEmbed as
      // a load-time side effect, which is available once onload fires.
      script.type = 'module';
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
          '<div class="datalogic-loading">Loading DataLogic playground...</div>';
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
        console.log('DataLogic playground initialized');
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

    // Debounce to avoid firing multiple times for a single page navigation
    let debounceTimer = null;

    const observer = new MutationObserver(() => {
      if (debounceTimer) return;
      debounceTimer = setTimeout(() => {
        debounceTimer = null;

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
        
        // Always check for new code tabs on navigation
        initCodeTabs();
      }, 100);
    });

    observer.observe(content, { childList: true, subtree: true });
  }

  /**
   * Initialize code tabs dynamically
   */
  function initCodeTabs() {
    const containers = document.querySelectorAll('.codetabs');
    const preferredLang = localStorage.getItem('datalogic-preferred-lang') || 'rust';

    containers.forEach((container) => {
      if (container.classList.contains('tabs-initialized')) return;
      container.classList.add('tabs-initialized');

      // Extract all code blocks inside the container
      const codeBlocks = Array.from(container.children).filter(el => el.tagName === 'PRE');
      if (codeBlocks.length === 0) return;

      // Map code block class to friendly labels
      const langMapping = {
        'language-rust': { id: 'rust', label: 'Rust' },
        'language-javascript': { id: 'javascript', label: 'JavaScript' },
        'language-typescript': { id: 'typescript', label: 'TypeScript' },
        'language-python': { id: 'python', label: 'Python' },
        'language-go': { id: 'go', label: 'Go' },
        'language-java': { id: 'java', label: 'Java' },
        'language-csharp': { id: 'csharp', label: 'C# / .NET' },
        'language-php': { id: 'php', label: 'PHP' }
      };

      // Create Tab Menu header
      const menu = document.createElement('div');
      menu.className = 'tab-menu';
      
      let activeTabId = preferredLang;
      let matchedTab = false;

      codeBlocks.forEach((block, idx) => {
        const codeClass = block.querySelector('code')?.className || '';
        const match = Object.keys(langMapping).find(k => codeClass.includes(k));
        const langInfo = match ? langMapping[match] : { id: `lang-${idx}`, label: `Lang ${idx}` };
        
        block.setAttribute('data-lang-id', langInfo.id);
        block.classList.add('tab-content');

        const btn = document.createElement('button');
        btn.className = 'tab-btn';
        btn.textContent = langInfo.label;
        btn.setAttribute('data-target-id', langInfo.id);
        
        btn.addEventListener('click', (e) => {
          switchTab(container, langInfo.id);
        });

        menu.appendChild(btn);
        
        if (langInfo.id === preferredLang) {
          matchedTab = true;
        }
      });

      // Default fallback if preference doesn't match any tab
      if (!matchedTab) {
        const firstBlock = codeBlocks[0];
        activeTabId = firstBlock.getAttribute('data-lang-id');
      }

      container.insertBefore(menu, codeBlocks[0]);
      switchTab(container, activeTabId);
    });
  }

  /**
   * Switch the active tab in a container and synchronize preferences
   */
  function switchTab(container, langId) {
    // Save preference
    localStorage.setItem('datalogic-preferred-lang', langId);
    
    // Synchronize ALL tab groups on the page
    document.querySelectorAll('.codetabs').forEach((grp) => {
      const btns = grp.querySelectorAll('.tab-btn');
      const blocks = grp.querySelectorAll('.tab-content');
      
      let hasTargetTab = Array.from(blocks).some(b => b.getAttribute('data-lang-id') === langId);
      let targetId = hasTargetTab ? langId : (blocks[0]?.getAttribute('data-lang-id') || '');

      btns.forEach(btn => {
        btn.classList.toggle('active', btn.getAttribute('data-target-id') === targetId);
      });

      blocks.forEach(block => {
        const isVisible = block.getAttribute('data-lang-id') === targetId;
        block.style.display = isVisible ? 'block' : 'none';
      });
    });
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

    // Initialize code tabs
    initCodeTabs();

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
