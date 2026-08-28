import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import FloatingWindow from "./FloatingWindow";
import QuickAddWindow from "./QuickAddWindow";
import UpdateProgressWindow from "./UpdateProgressWindow";
import "./styles.css";
import "./recovery.css";

const route = window.location.hash;
createRoot(document.getElementById("root")!).render(
  <StrictMode>
    {route.includes("update-progress") ? <UpdateProgressWindow /> : route.includes("floating") ? <FloatingWindow /> : route.includes("quick-add") ? <QuickAddWindow /> : <App />}
  </StrictMode>
);
