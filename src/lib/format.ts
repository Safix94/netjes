/** "1 bestand", "431 bestanden": enkelvoud bij precies 1, anders meervoud, zonder duizendtalscheiding. */
export function countLabel(n: number, singular: string, plural: string): string {
  return `${n} ${n === 1 ? singular : plural}`;
}
