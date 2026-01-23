import { memo } from 'react';

interface CollapseToggleButtonProps {
  isCollapsed: boolean;
  onClick: (e: React.MouseEvent) => void;
  /**
   * Style variant: 'header' for node headers (white text), 'cell' for cell rows (tertiary text).
   * Defaults to 'header'.
   */
  variant?: 'header' | 'cell';
}

/**
 * Reusable collapse/expand toggle button for nodes and cell rows.
 */
export const CollapseToggleButton = memo(function CollapseToggleButton({
  isCollapsed,
  onClick,
  variant = 'header',
}: CollapseToggleButtonProps) {
  return (
    <button
      className={variant === 'cell' ? 'cell-collapse-toggle' : 'collapse-toggle'}
      onClick={onClick}
      title={isCollapsed ? 'Expand' : 'Collapse'}
    >
      {isCollapsed ? '▶' : '▼'}
    </button>
  );
});
