import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { TypingPanel } from "./TypingPanel";
import { MainWindow } from "./MainWindow";
import "./styles.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>{getCurrentWindow().label === "typing" ? <TypingPanel /> : <MainWindow />}</StrictMode>,
);
