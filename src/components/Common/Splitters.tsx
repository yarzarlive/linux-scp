import React, { useCallback, useEffect, useState } from "react";

interface HorizontalSplitterProps {
  onResize: (deltaX: number) => void;
}

export const HorizontalSplitter: React.FC<HorizontalSplitterProps> = ({ onResize }) => {
  const [isDragging, setIsDragging] = useState(false);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setIsDragging(true);
  }, []);

  useEffect(() => {
    if (!isDragging) return;

    const handleMouseMove = (e: MouseEvent) => {
      onResize(e.movementX);
    };

    const handleMouseUp = () => {
      setIsDragging(false);
    };

    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);

    return () => {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
    };
  }, [isDragging, onResize]);

  return (
    <div
      role="separator"
      aria-orientation="vertical"
      onMouseDown={handleMouseDown}
      className={`w-1 hover:w-1.5 -mx-0.5 z-20 cursor-col-resize transition-all ${
        isDragging ? "bg-commander-accent" : "bg-commander-border hover:bg-commander-accent/70"
      }`}
      title="Drag to resize panes"
    />
  );
};

interface VerticalSplitterProps {
  onResize: (deltaY: number) => void;
}

export const VerticalSplitter: React.FC<VerticalSplitterProps> = ({ onResize }) => {
  const [isDragging, setIsDragging] = useState(false);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setIsDragging(true);
  }, []);

  useEffect(() => {
    if (!isDragging) return;

    const handleMouseMove = (e: MouseEvent) => {
      onResize(e.movementY);
    };

    const handleMouseUp = () => {
      setIsDragging(false);
    };

    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);

    return () => {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
    };
  }, [isDragging, onResize]);

  return (
    <div
      role="separator"
      aria-orientation="horizontal"
      onMouseDown={handleMouseDown}
      className={`h-1 hover:h-1.5 -my-0.5 z-20 cursor-row-resize transition-all ${
        isDragging ? "bg-commander-accent" : "bg-commander-border hover:bg-commander-accent/70"
      }`}
      title="Drag to resize transfer queue"
    />
  );
};
