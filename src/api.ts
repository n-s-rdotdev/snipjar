import { invoke } from "@tauri-apps/api/core";
import type { Entry, EntryInput, EntrySummary, PasteResult } from "./types";

export async function searchEntries(query: string): Promise<EntrySummary[]> {
  return invoke<EntrySummary[]>("search_entries", { query });
}

export async function getRecentEntries(): Promise<EntrySummary[]> {
  return invoke<EntrySummary[]>("get_recent_entries");
}

export async function createEntry(input: EntryInput): Promise<Entry> {
  return invoke<Entry>("create_entry", { input });
}

export async function getEntry(id: string): Promise<Entry> {
  return invoke<Entry>("get_entry", { id });
}

export async function updateEntry(id: string, input: EntryInput): Promise<Entry> {
  return invoke<Entry>("update_entry", { id, input });
}

export async function deleteEntry(id: string): Promise<void> {
  return invoke<void>("delete_entry", { id });
}

export async function copyEntry(id: string): Promise<PasteResult> {
  return invoke<PasteResult>("copy_entry", { id });
}

export async function pasteEntry(id: string): Promise<PasteResult> {
  return invoke<PasteResult>("paste_entry", { id });
}
