export default function EmptyState({
  title,
  text,
  action,
}: {
  title: string;
  text: string;
  action?: React.ReactNode;
}) {
  return (
    <div className="empty card">
      <div className="ring" aria-hidden="true" />
      <h2>{title}</h2>
      <p style={{ maxWidth: 420 }}>{text}</p>
      {action}
    </div>
  );
}
