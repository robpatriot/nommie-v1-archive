interface CardProps {
  card: string;
  className?: string;
}

export default function Card({ card, className = '' }: CardProps) {
  // Parse card string (e.g., "8H", "AS", "10D")
  const rank = card.slice(0, -1);
  const suit = card.slice(-1);

  const getSuitSymbol = (suit: string) => {
    switch (suit) {
      case 'H':
        return '♥';
      case 'D':
        return '♦';
      case 'C':
        return '♣';
      case 'S':
        return '♠';
      default:
        return suit;
    }
  };

  const getSuitColor = (suit: string) => {
    return suit === 'H' || suit === 'D' ? 'text-red-600' : 'text-gray-800';
  };

  return (
    <div
      className={`inline-block w-12 h-16 border border-gray-300 rounded-lg bg-white shadow-sm text-center ${className}`}
    >
      <div className="flex flex-col h-full justify-between p-1">
        <div className="text-xs font-bold text-gray-900">{rank}</div>
        <div className={`text-lg ${getSuitColor(suit)}`}>{getSuitSymbol(suit)}</div>
        <div className="text-xs font-bold text-gray-900 rotate-180">{rank}</div>
      </div>
    </div>
  );
}
