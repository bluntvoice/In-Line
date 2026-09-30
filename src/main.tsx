import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import FloatingWindow from "./FloatingWindow";
import QuickAddWindow from "./QuickAddWindow";
import UpdateProgressWindow from "./UpdateProgressWindow";
import UIFontProvider from "./components/UIFontProvider";
import "./styles.css";
import "./recovery.css";

const route = window.location.hash;
createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <UIFontProvider>
    {route.includes("update-progress") ? <UpdateProgressWindow /> : route.includes("floating") ? <FloatingWindow /> : route.includes("quick-add") ? <QuickAddWindow /> : <App />}
    </UIFontProvider>
  </StrictMode>
);
