export default function Logo({ withText = true }: { withText?: boolean }) {
  return (
    <span className="inline-flex items-center gap-2.5">
      <svg width="26" height="26" viewBox="0 0 26 26" fill="none" aria-hidden="true">
        <rect width="26" height="26" rx="7" fill="#1c1813" />
        <rect x="6" y="7.5" width="14" height="2" rx="1" fill="#faf8f3" />
        <rect x="6" y="12" width="14" height="2" rx="1" fill="#faf8f3" />
        <rect x="6" y="16.5" width="9" height="2" rx="1" fill="#b8451d" />
      </svg>
      {withText && (
        <span className="font-serif text-[17px] font-semibold tracking-tight text-ink">
          md-manager
        </span>
      )}
    </span>
  );
}
