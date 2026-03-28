export type Entry = {
  id: string;
  key: string;
  value: string;
  tags: string[];
  createdAt: string;
  updatedAt: string;
};

export type EntrySummary = {
  id: string;
  key: string;
  tags: string[];
  updatedAt: string;
};

export type EntryInput = {
  key: string;
  value: string;
  tags: string[];
};

export type PasteResult = {
  mode: "pasted" | "copied_only";
  message: string;
};
