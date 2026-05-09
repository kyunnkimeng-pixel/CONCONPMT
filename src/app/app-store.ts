import { create } from "zustand";

type AppShellStatus = "scaffolded";

interface AppState {
  productName: "PMTCONCON Studio";
  status: AppShellStatus;
}

export const useAppStore = create<AppState>(() => ({
  productName: "PMTCONCON Studio",
  status: "scaffolded",
}));
