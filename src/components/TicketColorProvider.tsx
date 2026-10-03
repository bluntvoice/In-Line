import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { api } from "../api";
import { applyTicketColors, DEFAULT_TICKET_COLORS, resolveTicketColors, type TicketColors } from "../lib/ticket-colors";

const TicketColorContext = createContext<{
  colors: TicketColors;
  unavailable: boolean;
  save: (colors: TicketColors) => Promise<void>;
}>({ colors: DEFAULT_TICKET_COLORS, unavailable: false, save: async () => { throw new Error("编号配色尚未初始化"); } });
export const useTicketColors = () => useContext(TicketColorContext);

export default function TicketColorProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState({ colors: DEFAULT_TICKET_COLORS, unavailable: false });
  const request = useRef(0);
  useEffect(() => { applyTicketColors(state.colors); }, [state.colors]);
  useEffect(() => {
    let active = true;
    const refresh = async () => {
      const id = ++request.current;
      try {
        const stored = await api.getTicketColors();
        if (active && id === request.current) setState({ colors: resolveTicketColors(stored), unavailable: false });
      } catch {
        if (active && id === request.current) setState({ colors: { ...DEFAULT_TICKET_COLORS }, unavailable: true });
      }
    };
    void refresh();
    const dispose = api.onDataChanged(() => void refresh());
    return () => { active = false; dispose(); };
  }, []);
  const save = async (colors: TicketColors) => {
    await api.setSetting("ticket_colors", JSON.stringify(colors));
    ++request.current;
    setState({ colors, unavailable: false });
  };
  return <TicketColorContext.Provider value={{ ...state, save }}>{children}</TicketColorContext.Provider>;
}
