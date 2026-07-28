import { invokeCommand } from "@/lib/tauri";
import type {
  AiWebHandoffHistoryItem,
  AiWebHandoffMaintenanceReport,
  AiWebHandoffStorageStatus,
} from "@/features/ai-handoff-history/types";

export function listRecentAiWebHandoffs(limit = 30) {
  return invokeCommand<AiWebHandoffHistoryItem[]>(
    "list_recent_ai_web_handoffs",
    { limit },
  );
}

export function getAiWebHandoffStorageStatus() {
  return invokeCommand<AiWebHandoffStorageStatus>(
    "get_ai_web_handoff_storage_status",
  );
}

export function runAiWebHandoffMaintenance() {
  return invokeCommand<AiWebHandoffMaintenanceReport>(
    "run_ai_web_handoff_maintenance",
  );
}