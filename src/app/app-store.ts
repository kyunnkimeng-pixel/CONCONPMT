import { create } from "zustand";

interface AppState {
  productName: "PMTCONCON Studio";
}

export const useAppStore = create<AppState>(() => ({
  productName: "PMTCONCON Studio",
}));
