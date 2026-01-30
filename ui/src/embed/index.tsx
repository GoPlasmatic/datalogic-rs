/**
 * DataLogic Embed - Standalone bundle for embedding in static HTML pages
 *
 * This module exposes a global `DataLogicEmbed` object that can be used to render
 * DataLogic visual widgets and playgrounds in mdBook or any static HTML page.
 *
 * Usage:
 * 1. Load React and ReactDOM from CDN
 * 2. Load this bundle (datalogic-embed.js)
 * 3. Call DataLogicEmbed.init() to auto-render all widgets
 *
 * Or manually:
 * - DataLogicEmbed.renderWidget(element, { logic, data, mode })
 * - DataLogicEmbed.renderPlayground(element)
 */

import React from 'react';
import ReactDOM from 'react-dom/client';

// Import styles - include all component CSS (but NOT index.css which has global styles)
import '@xyflow/react/dist/style.css';
// NOTE: index.css is NOT imported here because it has global styles that affect the entire page
import '../components/logic-editor/styles/nodes.css';
import '../components/logic-editor/LogicEditor.css';
import '../components/logic-editor/debugger-controls/DebuggerControls.css';
import '../components/debug-panel/DebugPanel.css';
import '../embed.css';

import { Widget, type WidgetProps } from './Widget';
import { Playground, type PlaygroundProps } from './Playground';
import { parseDataAttributes } from './utils';

// Track mounted roots for cleanup
const mountedRoots = new Map<Element, ReactDOM.Root>();

/**
 * Unmount a React root from an element
 */
function unmountElement(element: Element) {
  const root = mountedRoots.get(element);
  if (root) {
    root.unmount();
    mountedRoots.delete(element);
  }
}

const DataLogicEmbed = {
  /**
   * Render a widget into an element
   */
  renderWidget(element: Element, props: Partial<WidgetProps> = {}) {
    // Unmount existing root if present
    unmountElement(element);

    // Parse attributes and merge with props
    const parsedProps = parseDataAttributes(element);
    const finalProps = { ...parsedProps, ...props };

    // Create root and render
    const root = ReactDOM.createRoot(element);
    mountedRoots.set(element, root);
    root.render(
      <React.StrictMode>
        <Widget {...finalProps} />
      </React.StrictMode>
    );
  },

  /**
   * Render the full playground into an element
   */
  renderPlayground(element: Element, props: Partial<PlaygroundProps> = {}) {
    // Unmount existing root if present
    unmountElement(element);

    // Parse componentMode from data attributes if not provided
    const componentModeAttr = (element.getAttribute('data-component-mode') || element.getAttribute('data-datalogic-component-mode')) as 'debugger' | 'visualizer' | null;
    const finalProps: PlaygroundProps = {
      componentMode: props.componentMode || componentModeAttr || 'debugger',
    };

    // Create root and render
    const root = ReactDOM.createRoot(element);
    mountedRoots.set(element, root);
    root.render(
      <React.StrictMode>
        <Playground {...finalProps} />
      </React.StrictMode>
    );
  },

  /**
   * Auto-render all widgets on the page
   * Widgets: elements with [data-datalogic], [data-logic], or .playground-widget class
   * Playground: element with #datalogic-playground or [data-datalogic-playground]
   */
  renderWidgets() {
    // Render playground if present
    const playground = document.querySelector('#datalogic-playground, [data-datalogic-playground]');
    if (playground) {
      this.renderPlayground(playground);
    }

    // Render widgets - support multiple selector patterns
    const widgets = document.querySelectorAll('[data-datalogic]:not([data-datalogic-playground]), .playground-widget, [data-logic]:not([data-datalogic-playground])');
    widgets.forEach((widget) => {
      if (!widget.classList.contains('datalogic-initialized')) {
        this.renderWidget(widget);
      }
    });
  },

  /**
   * Initialize - render widgets and set up observers for page changes
   * Call this once after loading the script
   */
  init() {
    // Render existing widgets
    this.renderWidgets();

    // Watch for page changes (mdBook navigation)
    const content = document.getElementById('content');
    if (content) {
      const observer = new MutationObserver(() => {
        // Clean up unmounted widgets
        mountedRoots.forEach((root, element) => {
          if (!document.body.contains(element)) {
            root.unmount();
            mountedRoots.delete(element);
          }
        });

        // Render new widgets
        this.renderWidgets();
      });

      observer.observe(content, { childList: true, subtree: true });
    }
  },

  /**
   * Cleanup all mounted widgets
   */
  cleanup() {
    mountedRoots.forEach((root) => root.unmount());
    mountedRoots.clear();
  },
};

// Expose globally
declare global {
  interface Window {
    DataLogicEmbed: typeof DataLogicEmbed;
  }
}

window.DataLogicEmbed = DataLogicEmbed;

export default DataLogicEmbed;
