import React from 'react';
import type { LucideIcon } from 'lucide-react';
import {
  Scale,
  Diamond,
  Calculator,
  Repeat,
  Type,
  Box,
  GitMerge,
  GitBranch,
  Text,
  Hash,
  ToggleLeft,
  ToggleRight,
  Check,
  X,
  Ban,
  List,
  Calendar,
  Cog,
  Database,
  Boxes,
  CircleHelp,
  CircleX,
  GitCommitHorizontal,
  Search,
  Divide,
  Quote,
  Braces,
  Binary,
  Layers,
  Clock,
  CircleAlert,
  ArrowUp,
  Tag,
} from 'lucide-react';
import type { IconName } from './icons';

// Map icon names to Lucide components
const ICON_COMPONENTS: Record<IconName, LucideIcon> = {
  'scale': Scale,
  'diamond': Diamond,
  'calculator': Calculator,
  'repeat': Repeat,
  'type': Type,
  'box': Box,
  'git-merge': GitMerge,
  'git-branch': GitBranch,
  'text': Text,
  'hash': Hash,
  'toggle-left': ToggleLeft,
  'toggle-right': ToggleRight,
  'check': Check,
  'x': X,
  'ban': Ban,
  'list': List,
  'calendar': Calendar,
  'cog': Cog,
  'database': Database,
  'boxes': Boxes,
  'circle-help': CircleHelp,
  'circle-x': CircleX,
  'git-commit-horizontal': GitCommitHorizontal,
  'search': Search,
  'divide': Divide,
  'quote': Quote,
  'braces': Braces,
  'binary': Binary,
  'layers': Layers,
  'clock': Clock,
  'alert-circle': CircleAlert,
  'arrow-up': ArrowUp,
  'tag': Tag,
};

// Render an icon by name
interface IconProps {
  name: IconName;
  size?: number;
  className?: string;
  style?: React.CSSProperties;
}

export function Icon({ name, size = 14, className, style }: IconProps): React.ReactElement {
  const IconComponent = ICON_COMPONENTS[name];
  return <IconComponent size={size} className={className} style={style} />;
}
