import { useEffect, useCallback } from 'react';
import {
  Bug,
  SkipBack,
  ChevronLeft,
  Play,
  Pause,
  ChevronRight,
  SkipForward,
} from 'lucide-react';
import { useDebuggerContext } from '../context';
import './DebuggerControls.css';

export function DebuggerControlsInline() {
  return <DebuggerControlsBase variant="inline" />;
}

export function DebuggerControls() {
  return <DebuggerControlsBase variant="floating" />;
}

function DebuggerControlsBase({ variant = 'floating' }: { variant?: 'inline' | 'floating' }) {
  const {
    state,
    play,
    pause,
    reset,
    stepForward,
    stepBackward,
    goToStep,
    setSpeed,
  } = useDebuggerContext();

  const { steps, currentStepIndex, playbackState, playbackSpeed } = state;
  const isPlaying = playbackState === 'playing';
  const totalSteps = steps.length;
  // At -1 = initial state (plain visualizer, no step active)
  const isAtInitial = currentStepIndex < 0;
  const isAtStart = isAtInitial;
  const isAtEnd = currentStepIndex >= totalSteps - 1;

  // Keyboard shortcuts
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      // Ignore if user is typing in an input
      if (
        e.target instanceof HTMLInputElement ||
        e.target instanceof HTMLTextAreaElement
      ) {
        return;
      }

      switch (e.key) {
        case ' ':
          e.preventDefault();
          if (isPlaying) {
            pause();
          } else {
            play();
          }
          break;
        case 'ArrowLeft':
          e.preventDefault();
          stepBackward();
          break;
        case 'ArrowRight':
          e.preventDefault();
          stepForward();
          break;
        case 'Home':
          e.preventDefault();
          reset();
          break;
        case 'End':
          e.preventDefault();
          goToStep(totalSteps - 1);
          break;
      }
    },
    [isPlaying, pause, play, stepBackward, stepForward, reset, goToStep, totalSteps]
  );

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [handleKeyDown]);

  // Convert speed (ms) to display value (inverted for intuitive slider)
  // Lower ms = faster, but slider should go left-to-right as faster
  const speedToSlider = (ms: number) => 2100 - ms; // Range: 100-2000 -> 2000-100
  const sliderToSpeed = (val: number) => 2100 - val;

  if (totalSteps === 0) {
    return null;
  }

  return (
    <div className={`debugger-controls--${variant}`}>
      <div className="debugger-controls-inner">
        {/* Bug icon indicator */}
        <div className="debugger-icon">
          <Bug size={variant === 'inline' ? 15 : 18} />
        </div>

        {/* Navigation buttons */}
        <div className="debugger-buttons">
          <button
            className="debugger-btn"
            onClick={reset}
            disabled={isAtStart}
            title="Reset to start (Home)"
          >
            <SkipBack size={16} />
          </button>

          <button
            className="debugger-btn"
            onClick={stepBackward}
            disabled={isAtStart}
            title="Step backward (Left Arrow)"
          >
            <ChevronLeft size={18} />
          </button>

          <button
            className="debugger-btn debugger-btn-primary"
            onClick={isPlaying ? pause : play}
            title={isPlaying ? 'Pause (Space)' : 'Play (Space)'}
          >
            {isPlaying ? <Pause size={18} /> : <Play size={18} />}
          </button>

          <button
            className="debugger-btn"
            onClick={stepForward}
            disabled={isAtEnd}
            title="Step forward (Right Arrow)"
          >
            <ChevronRight size={18} />
          </button>

          <button
            className="debugger-btn"
            onClick={() => goToStep(totalSteps - 1)}
            disabled={isAtEnd}
            title="Jump to end (End)"
          >
            <SkipForward size={16} />
          </button>
        </div>

        {/* Step counter: 0/N at initial, then 1/N .. N/N when stepping */}
        <div className="debugger-step-counter">
          <span className="step-current">{isAtInitial ? 0 : currentStepIndex + 1}</span>
          <span className="step-separator">/</span>
          <span className="step-total">{totalSteps}</span>
        </div>

        {/* Speed control */}
        <div className="debugger-speed">
          <label className="speed-label">Speed</label>
          <input
            type="range"
            className="speed-slider"
            min={100}
            max={2000}
            step={100}
            value={speedToSlider(playbackSpeed)}
            onChange={(e) => setSpeed(sliderToSpeed(parseInt(e.target.value, 10)))}
            title={`${playbackSpeed}ms per step`}
          />
        </div>
      </div>
    </div>
  );
}
