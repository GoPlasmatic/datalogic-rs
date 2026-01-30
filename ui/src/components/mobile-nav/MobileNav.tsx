import { GitBranch, Code } from 'lucide-react';
import './MobileNav.css';

export type MobileTab = 'visual' | 'code';

interface MobileNavProps {
  activeTab: MobileTab;
  onTabChange: (tab: MobileTab) => void;
}

const tabs: { id: MobileTab; label: string; icon: typeof Code }[] = [
  { id: 'visual', label: 'Visual', icon: GitBranch },
  { id: 'code', label: 'Code', icon: Code },
];

export function MobileNav({ activeTab, onTabChange }: MobileNavProps) {
  return (
    <nav className="mobile-nav">
      {tabs.map(({ id, label, icon: Icon }) => (
        <button
          key={id}
          className={`mobile-nav-tab ${activeTab === id ? 'active' : ''}`}
          onClick={() => onTabChange(id)}
          aria-selected={activeTab === id}
        >
          <Icon size={20} />
          <span>{label}</span>
        </button>
      ))}
    </nav>
  );
}
