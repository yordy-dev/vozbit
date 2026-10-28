import React from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import Overlay from "./Overlay";
import "./style.css";
import "./refinements.css";
import "./polish.css";
import "./redesign.css";

const isOverlay = new URLSearchParams(window.location.search).has("overlay");
document.documentElement.classList.toggle("overlay-document", isOverlay);
createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {isOverlay ? <Overlay /> : <App />}
  </React.StrictMode>,
);
