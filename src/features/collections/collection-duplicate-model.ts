export interface CollectionDuplicateAvailabilityInput {
  collectionId: string | null;
  isDuplicating: boolean;
  isImporting: boolean;
}

export function canStartCollectionDuplicate({
  collectionId,
  isDuplicating,
  isImporting,
}: CollectionDuplicateAvailabilityInput) {
  return Boolean(collectionId) && !isDuplicating && !isImporting;
}
