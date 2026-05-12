export interface ManualSliceDraft {
  sliceId: string;
  name: string;
  x: number;
  y: number;
  w: number;
  h: number;
  orderIndex: number;
  include: boolean;
  notes: string;
}

export function isValidManualSlice(slice: ManualSliceDraft) {
  return slice.w > 0 && slice.h > 0 && slice.x >= 0 && slice.y >= 0;
}
