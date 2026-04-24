import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

function App() {
  const [hasStarted, setHasStarted] = useState(false);

  useEffect(() => {
    if (!hasStarted) return;

    const handleKeyDown = (e) => {
      invoke("send_input", { input: { keyDown: e.key } });
    };

    const handleKeyUp = (e) => {
      invoke("send_input", { input: { keyUp: e.key } });
    };

    const handleMouseDown = (e) => {
      invoke("send_input", { input: { mouseDown: e.button } });
    };

    const handleMouseUp = (e) => {
      invoke("send_input", { input: { mouseUp: e.button } });
    };

    const handleMouseMove = (e) => {
      // We use movementX/Y for FPS-style look
      if (document.pointerLockElement) {
        invoke("send_input", { 
          input: { mouseMove: { dx: e.movementX, dy: e.movementY } } 
        });
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("keyup", handleKeyUp);
    window.addEventListener("mousedown", handleMouseDown);
    window.addEventListener("mouseup", handleMouseUp);
    window.addEventListener("mousemove", handleMouseMove);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("keyup", handleKeyUp);
      window.removeEventListener("mousedown", handleMouseDown);
      window.removeEventListener("mouseup", handleMouseUp);
      window.removeEventListener("mousemove", handleMouseMove);
    };
  }, [hasStarted]);

  if (!hasStarted) {
    return (
      <main className="start-screen">
        <div className="start-content">
          <h1>Voxel Project</h1>
          <p>Tauri 2.0 + Bevy 0.18 Integration</p>
          <button className="start-button" onClick={() => setHasStarted(true)}>
            START ENGINE
          </button>
        </div>
      </main>
    );
  }

  return (
    <main className="container" onClick={(e) => e.currentTarget.requestPointerLock()}>
      <div className="overlay-ui">
        <h1>Engine Active</h1>
        <p>Click anywhere to lock mouse and move.</p>
        <button className="back-button" onClick={(e) => {
          e.stopPropagation();
          setHasStarted(false);
          document.exitPointerLock();
        }}>
          Back
        </button>
      </div>
    </main>
  );
}

export default App;
