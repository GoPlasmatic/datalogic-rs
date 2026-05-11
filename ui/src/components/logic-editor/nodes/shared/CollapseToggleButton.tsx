import { memo } from 'react';

interface CollapseToggleButtonProps {
  isCollapsed: boolean;
  onClick: (e: React.MouseEvent) => void;
}

/**
 * Reusable collapse/expand toggle button for node headers.
 */
export const CollapseToggleButton = memo(function CollapseToggleButton({
  isCollapsed,
  onClick,
}: CollapseToggleButtonProps) {
  return (
    <button
      className="collapse-toggle"
      onClick={onClick}
      title={isCollapsed ? 'Expand' : 'Collapse'}
    >
      {isCollapsed ? '▶' : '▼'}
    </button>
  );
});
