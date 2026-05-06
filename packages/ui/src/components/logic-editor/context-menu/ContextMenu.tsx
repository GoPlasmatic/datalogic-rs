/**
 * ContextMenu Component
 *
 * A generic context menu component with support for:
 * - Positioning based on click coordinates
 * - Close on click outside or Escape key
 * - Keyboard navigation (arrow keys)
 * - Submenu support
 */

import { memo, useEffect, useRef, useCallback, useState, type ReactNode } from 'react';
import { ChevronRight } from 'lucide-react';

export interface MenuItemConfig {
  id: string;
  label: string;
  icon?: ReactNode;
  shortcut?: string;
  disabled?: boolean;
  danger?: boolean;
  onClick?: () => void;
  submenu?: MenuItemConfig[];
}

export interface ContextMenuProps {
  /** X position (screen coordinates) */
  x: number;
  /** Y position (screen coordinates) */
  y: number;
  /** Menu items to display */
  items: MenuItemConfig[];
  /** Called when menu should close */
  onClose: () => void;
}

export const ContextMenu = memo(function ContextMenu({
  x,
  y,
  items,
  onClose,
}: ContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);
  const [focusedIndex, setFocusedIndex] = useState(-1);
  const [openSubmenuId, setOpenSubmenuId] = useState<string | null>(null);

  // Adjust position to keep menu on screen
  const [position, setPosition] = useState({ x, y });

  useEffect(() => {
    if (menuRef.current) {
      const rect = menuRef.current.getBoundingClientRect();
      const viewportWidth = window.innerWidth;
      const viewportHeight = window.innerHeight;

      let adjustedX = x;
      let adjustedY = y;

      // Adjust horizontal position if menu goes off screen
      if (x + rect.width > viewportWidth - 8) {
        adjustedX = viewportWidth - rect.width - 8;
      }

      // Adjust vertical position if menu goes off screen
      if (y + rect.height > viewportHeight - 8) {
        adjustedY = viewportHeight - rect.height - 8;
      }

      setPosition({ x: adjustedX, y: adjustedY });
    }
  }, [x, y]);

  // Handle click outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };

    // Use capture phase to handle click before other handlers
    document.addEventListener('mousedown', handleClickOutside, true);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside, true);
    };
  }, [onClose]);

  // Handle keyboard navigation
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      const enabledItems = items.filter((item) => !item.disabled && item.id !== 'divider');

      switch (e.key) {
        case 'Escape':
          e.preventDefault();
          onClose();
          break;

        case 'ArrowDown':
          e.preventDefault();
          setFocusedIndex((prev) => {
            const next = prev + 1;
            return next >= enabledItems.length ? 0 : next;
          });
          break;

        case 'ArrowUp':
          e.preventDefault();
          setFocusedIndex((prev) => {
            const next = prev - 1;
            return next < 0 ? enabledItems.length - 1 : next;
          });
          break;

        case 'ArrowRight': {
          e.preventDefault();
          const focusedItem = enabledItems[focusedIndex];
          if (focusedItem?.submenu) {
            setOpenSubmenuId(focusedItem.id);
          }
          break;
        }

        case 'ArrowLeft':
          e.preventDefault();
          setOpenSubmenuId(null);
          break;

        case 'Enter':
        case ' ':
          e.preventDefault();
          if (focusedIndex >= 0 && focusedIndex < enabledItems.length) {
            const item = enabledItems[focusedIndex];
            if (item.submenu) {
              setOpenSubmenuId(item.id);
            } else if (item.onClick) {
              item.onClick();
              onClose();
            }
          }
          break;
      }
    },
    [items, focusedIndex, onClose]
  );

  useEffect(() => {
    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('keydown', handleKeyDown);
    };
  }, [handleKeyDown]);

  // Focus menu on mount
  useEffect(() => {
    menuRef.current?.focus();
  }, []);

  const handleItemClick = useCallback(
    (item: MenuItemConfig) => {
      if (item.disabled) return;

      if (item.submenu) {
        setOpenSubmenuId(openSubmenuId === item.id ? null : item.id);
      } else if (item.onClick) {
        item.onClick();
        onClose();
      }
    },
    [openSubmenuId, onClose]
  );

  const handleItemMouseEnter = useCallback(
    (item: MenuItemConfig, index: number) => {
      setFocusedIndex(index);
      if (item.submenu) {
        setOpenSubmenuId(item.id);
      } else {
        setOpenSubmenuId(null);
      }
    },
    []
  );

  // Filter out consecutive dividers and dividers at start/end
  const filteredItems = items.reduce<MenuItemConfig[]>((acc, item, index) => {
    if (item.id === 'divider') {
      // Skip divider at start
      if (acc.length === 0) return acc;
      // Skip consecutive dividers
      if (acc[acc.length - 1]?.id === 'divider') return acc;
      // Skip divider at end
      if (index === items.length - 1) return acc;
    }
    acc.push(item);
    return acc;
  }, []);

  // Remove trailing dividers
  while (filteredItems.length > 0 && filteredItems[filteredItems.length - 1]?.id === 'divider') {
    filteredItems.pop();
  }

  let enabledIndex = -1;

  return (
    <div
      ref={menuRef}
      className="dl-context-menu"
      style={{
        left: position.x,
        top: position.y,
      }}
      tabIndex={-1}
      role="menu"
    >
      {filteredItems.map((item, itemIndex) => {
        if (item.id === 'divider') {
          return <div key={`divider-${itemIndex}`} className="dl-context-menu-divider" role="separator" />;
        }

        if (!item.disabled) {
          enabledIndex++;
        }
        const currentEnabledIndex = enabledIndex;
        const isFocused = !item.disabled && focusedIndex === currentEnabledIndex;

        return (
          <div key={item.id} className={item.submenu ? 'dl-context-menu-submenu' : undefined}>
            <button
              type="button"
              className={[
                'dl-context-menu-item',
                item.danger && 'dl-context-menu-item--danger',
                isFocused && 'dl-context-menu-item--focused',
              ]
                .filter(Boolean)
                .join(' ')}
              disabled={item.disabled}
              onClick={() => handleItemClick(item)}
              onMouseEnter={() => handleItemMouseEnter(item, currentEnabledIndex)}
              role="menuitem"
              tabIndex={-1}
            >
              {item.icon && <span className="dl-context-menu-item-icon">{item.icon}</span>}
              <span className="dl-context-menu-item-label">{item.label}</span>
              {item.shortcut && <span className="dl-context-menu-item-shortcut">{item.shortcut}</span>}
              {item.submenu && (
                <span className="dl-context-menu-item-arrow">
                  <ChevronRight size={14} />
                </span>
              )}
            </button>

            {/* Render submenu */}
            {item.submenu && openSubmenuId === item.id && (
              <div className="dl-context-menu-submenu-content">
                <ContextMenuSubmenu items={item.submenu} onClose={onClose} />
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
});

// Submenu component with support for nested submenus
interface ContextMenuSubmenuProps {
  items: MenuItemConfig[];
  onClose: () => void;
}

const ContextMenuSubmenu = memo(function ContextMenuSubmenu({
  items,
  onClose,
}: ContextMenuSubmenuProps) {
  const [openSubmenuId, setOpenSubmenuId] = useState<string | null>(null);

  const handleItemClick = useCallback(
    (item: MenuItemConfig) => {
      if (item.disabled) return;
      if (item.submenu) {
        setOpenSubmenuId(openSubmenuId === item.id ? null : item.id);
      } else if (item.onClick) {
        item.onClick();
        onClose();
      }
    },
    [onClose, openSubmenuId]
  );

  const handleItemMouseEnter = useCallback((item: MenuItemConfig) => {
    if (item.submenu) {
      setOpenSubmenuId(item.id);
    } else {
      setOpenSubmenuId(null);
    }
  }, []);

  return (
    <div className="dl-context-menu" role="menu">
      {items.map((item, itemIndex) => {
        if (item.id === 'divider') {
          return <div key={`divider-${itemIndex}`} className="dl-context-menu-divider" role="separator" />;
        }

        return (
          <div key={item.id} className={item.submenu ? 'dl-context-menu-submenu' : undefined}>
            <button
              type="button"
              className={[
                'dl-context-menu-item',
                item.danger && 'dl-context-menu-item--danger',
              ]
                .filter(Boolean)
                .join(' ')}
              disabled={item.disabled}
              onClick={() => handleItemClick(item)}
              onMouseEnter={() => handleItemMouseEnter(item)}
              role="menuitem"
              tabIndex={-1}
            >
              {item.icon && <span className="dl-context-menu-item-icon">{item.icon}</span>}
              <span className="dl-context-menu-item-label">{item.label}</span>
              {item.shortcut && <span className="dl-context-menu-item-shortcut">{item.shortcut}</span>}
              {item.submenu && (
                <span className="dl-context-menu-item-arrow">
                  <ChevronRight size={14} />
                </span>
              )}
            </button>

            {/* Render nested submenu */}
            {item.submenu && openSubmenuId === item.id && (
              <div className="dl-context-menu-submenu-content">
                <ContextMenuSubmenu items={item.submenu} onClose={onClose} />
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
});
