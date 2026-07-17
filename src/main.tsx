import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import FloatingWindow from "./FloatingWindow";
import "./styles.css";
import "./recovery.css";

const route = window.location.hash;
createRoot(document.getElementById("root")!).render(
  <StrictMode>
    {route.includes("floating") ? <FloatingWindow /> : <App />}
  </StrictMode>
);
