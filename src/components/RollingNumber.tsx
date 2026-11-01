const DIGITS = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];

/**
 * Een getal als rollende cijfers: elk cijfer is een strook van 0 tot 9 die omhoog schuift tot het
 * juiste cijfer in beeld staat (de overgang van 280 ms staat in app.css). Enkel cijfers, geen
 * duizendtalscheiding. De sleutel is de plaatswaarde, zodat een cijfer blijft rollen als het
 * getal langer of korter wordt.
 */
export function RollingNumber({ value }: { value: number }) {
  const digits = String(Math.max(0, Math.trunc(value))).split("");
  return (
    <span className="rn" role="img" aria-label={String(value)}>
      {digits.map((digit, i) => (
        <span className="rd" data-testid="digit" aria-hidden="true" key={digits.length - i}>
          <span className="rs" style={{ transform: `translateY(-${digit}em)` }}>
            {DIGITS.map((d) => (
              <span key={d}>{d}</span>
            ))}
          </span>
        </span>
      ))}
    </span>
  );
}
