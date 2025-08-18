import Card from './Card';

interface PlayerHandProps {
  cards: string[];
  className?: string;
}

export default function PlayerHand({ cards, className = '' }: PlayerHandProps) {
  if (!cards || cards.length === 0) {
    return (
      <div className={`bg-white dark:bg-gray-800 shadow rounded-lg p-6 ${className}`}>
        <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">Your Hand</h3>
        <p className="text-gray-500 dark:text-gray-400 text-center py-8">No cards in hand</p>
      </div>
    );
  }

  return (
    <div className={`bg-white dark:bg-gray-800 shadow rounded-lg p-6 ${className}`}>
      <h3 className="text-lg font-semibold text-gray-900 dark:text-white mb-4">
        Your Hand ({cards.length} cards)
      </h3>
      <div className="flex flex-wrap gap-2">
        {cards.map((card, index) => (
          <Card key={index} card={card} />
        ))}
      </div>
    </div>
  );
}
