type Props = {
  type: 'date' | 'time' | 'datetime-local';
  value: string;
  emptyText: string;
  onChange: (value: string) => void;
  disabled?: boolean;
};

export function OptionalDateTimeInput({ type, value, emptyText, onChange, disabled }: Props) {
  return <span className="optional-date-time" data-empty={!value}>
    <input type={type} value={value} onChange={(event) => onChange(event.target.value)} disabled={disabled} />
    {!value && <span className="optional-date-time-placeholder" aria-hidden="true">{emptyText}</span>}
  </span>;
}
