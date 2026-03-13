---
name: explain
description: Explain code through a software craftsmanship lens. Use when the user asks how something works, why code is structured a certain way, or wants to understand a pattern. Teaches principles like single responsibility, naming, separation of concerns, and clean code.
---

When explaining code, follow this structure:

1. **What it does** — Summarize the purpose in one or two sentences. Use plain language, not just code terminology.

2. **How it works** — Walk through the logic step by step. Use an ASCII diagram if the flow involves multiple components or data transformations.

3. **Why it's done this way** — This is the most important part. Explain the design decisions and trade-offs:
   - What craftsmanship principle does this follow? (single responsibility, separation of concerns, explicit over implicit, etc.)
   - What alternatives exist and why were they not chosen?
   - What would happen if this was done differently?

4. **What could be improved** — If there are opportunities to make the code cleaner, simpler, or more expressive, mention them gently as learning points. Not everything needs improvement — say so when the code is already clean.

Keep explanations conversational. Use concrete examples from the codebase, not abstract theory. When referencing craftsmanship principles, connect them to the actual code rather than just naming them.

This is a learning project. The goal is to build understanding, not just deliver answers. Explain the "why" behind every "what".
