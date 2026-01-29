import { Eye, Bug, Pencil } from 'lucide-react';
import type { DataLogicEditorMode } from '../logic-editor/types';
import './ModeSelector.css';

interface ModeSelectorProps {
  mode: DataLogicEditorMode;
  onChange: (mode: DataLogicEditorMode) => void;
}

const MODE_CONFIG: Record<DataLogicEditorMode, { label: string; icon: typeof Eye; title: string; beta?: boolean }> = {
  visualize: {
    label: 'View',
    icon: Eye,
    title: 'Read-only visualization',
  },
  debug: {
    label: 'Debug',
    icon: Bug,
    title: 'Debug with step-through execution',
  },
  edit: {
    label: 'Edit',
    icon: Pencil,
    title: 'Visual Editor (Beta)',
    beta: true,
  },
};

const MODES: DataLogicEditorMode[] = ['visualize', 'debug', 'edit'];

export function ModeSelector({ mode, onChange }: ModeSelectorProps) {
  return (
    <div className="mode-selector" role="tablist" aria-label="Editor mode">
      {MODES.map((m) => {
        const config = MODE_CONFIG[m];
        const Icon = config.icon;
        const isActive = mode === m;

        return (
          <button
            key={m}
            role="tab"
            aria-selected={isActive}
            className={`mode-button ${isActive ? 'active' : ''}`}
            onClick={() => onChange(m)}
            title={config.title}
          >
            <Icon size={14} />
            <span>{config.label}</span>
            {config.beta && <span className="beta-badge">β</span>}
          </button>
        );
      })}
    </div>
  );
}

export type { ModeSelectorProps };

export default ModeSelector;
