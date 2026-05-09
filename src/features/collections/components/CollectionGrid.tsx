import { CollectionCard } from "@/features/collections/components/CollectionCard";
import type { CollectionSummary } from "@/features/collections/types";

interface CollectionGridProps {
  collections: CollectionSummary[];
  selectedCollectionId: string | null;
  onOpenCollection: (collectionId: string) => void;
  onRenameCollection: (collectionId: string, name: string) => void;
  onSelectCollection: (collectionId: string) => void;
}

export function CollectionGrid({
  collections,
  selectedCollectionId,
  onOpenCollection,
  onRenameCollection,
  onSelectCollection,
}: CollectionGridProps) {
  return (
    <section
      aria-label="디시콘 모음"
      className="grid grid-cols-[repeat(auto-fill,minmax(176px,1fr))] gap-4"
      role="listbox"
    >
      {collections.map((collection) => (
        <CollectionCard
          collection={collection}
          isSelected={collection.id === selectedCollectionId}
          key={collection.id}
          onOpen={() => onOpenCollection(collection.id)}
          onRename={(name) => onRenameCollection(collection.id, name)}
          onSelect={() => onSelectCollection(collection.id)}
        />
      ))}
    </section>
  );
}
