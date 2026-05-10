---
name: deep-interview-questions
description: Structured question bank for the deep-interview Socratic interview skill — organized by domain with probes, quality indicators, and clarity-dimension mapping
---

# Deep Interview Question Bank

Companion reference for the `deep-interview` skill. Questions are organized by domain, tagged with the clarity dimensions they target (Goal / Constraint / Criteria / Context), and include follow-up probes and answer-quality indicators.

**How to use:** During an interview round, select questions from the domain that maps to the weakest-dimension cluster for the active component. Each question lists its primary dimension target and secondary dimension boost.

**Legend:**
- **P** = primary dimension the question sharpens
- **S** = secondary dimension that also benefits
- **Round tag** = recommended minimum round (blank = any round)

---

## 1. Requirements Domain

### R1. Core Action Sequence
**Dimension:** Goal (P), Criteria (S)
**Question:** "Walk me through the single most important action a user performs in this system, step by step. What do they click/enter first, what happens next, and what is the final output they see?"
**Follow-up probes:**
- "Is that action performed once, periodically, or continuously?"
- "Does the user need to be authenticated before this action, or is it public?"
- "What happens if they stop halfway through — is partial progress saved?"
**Quality indicators:** Answer names a concrete first step, a sequence of 2-5 sub-steps, and a tangible output (screen, file, message). Vague answers mention feelings or goals without steps.

### R2. Entity Identification
**Dimension:** Goal (P), Constraint (S)
**Question:** "List the nouns in your description. For each one, tell me: is it something the user creates, something the system provides, or something from an external system?"
**Follow-up probes:**
- "Which noun is the core — if you could only have one, which would it be?"
- "Do any of these nouns have a lifecycle (created, updated, archived, deleted)?"
- "Are any of these nouns actually the same thing viewed from different angles?"
**Quality indicators:** Answer distinguishes user-created vs system-managed vs external entities. Strong answers also describe entity lifecycles. Weak answers conflate actions with nouns or name too many "core" entities.

### R3. User Mental Model
**Dimension:** Goal (P)
**Question:** "If you had to explain this product to a friend in one sentence, what would you say? Now — is there an existing product or metaphor it resembles?"
**Follow-up probes:**
- "What does it do like that product, and where does it diverge?"
- "Would a user of that existing product feel at home, or is this a deliberate subversion?"
- "What is the one thing this does that the comparable product does not?"
**Quality indicators:** Answer produces a crisp analogy (not "it's like Uber meets Notion but for dogs"). The divergence points should map to real requirements, not aspirational features.

### R4. Scope Boundary
**Dimension:** Constraint (P), Goal (S)
**Question:** "Name three things this product explicitly does NOT do. Not 'things we might add later' — things that are permanently out of scope."
**Follow-up probes:**
- "If a user asked for one of those out-of-scope features, what would the system say or do?"
- "Are any of these non-goals actually hard technical limits rather than scope decisions?"
- "Could any of these non-goals become in-scope without changing the core architecture?"
**Quality indicators:** Answer names concrete exclusions tied to the project's purpose, not generic exclusions ("we won't do blockchain"). Strong answers explain why each is excluded.

### R5. Stakeholder Map
**Dimension:** Constraint (P), Context (S)
**Question:** "Who are the distinct user roles or personas that interact with this system? For each, what is the one thing they care about most?"
**Follow-up probes:**
- "Does any role need to approve or gate what another role does?"
- "Are there roles that should never see certain data or features?"
- "Is there an admin role, and if so, what does admin override look like?"
**Quality indicators:** Answer names 2-4 distinct roles with clear differentiation. Each role's primary concern should be different. Weak answers list "user" and "admin" without explaining what each actually does.

### R6. Data Ownership
**Dimension:** Goal (P), Constraint (S)
**Question:** "Who owns the data in this system? If the product disappears tomorrow, what data belongs to the user vs. the platform vs. neither?"
**Follow-up probes:**
- "Can users export their data? In what format?"
- "Is any data shared across users, or is every user's data fully isolated?"
- "Are there regulatory constraints on where this data can be stored (region, encryption, retention)?"
**Quality indicators:** Answer clarifies data residency, exportability, and ownership model. Strong answers reference specific regulations or contractual obligations. Weak answers say "we store everything in the cloud."

### R7. Error Expectations
**Dimension:** Criteria (P), Constraint (S)
**Question:** "What does failure look like? Not bugs — I mean expected failures the system must handle gracefully. What can go wrong in normal operation?"
**Follow-up probes:**
- "If the external service you depend on is down, what does the user see?"
- "What is the user's recovery path from each failure?"
- "Are any failures silent (user doesn't notice) vs. loud (user must act)?"
**Quality indicators:** Answer identifies 3+ realistic failure modes with user-facing consequences. Strong answers distinguish recoverable vs. catastrophic failures and specify the user's expected behavior in each case.

### R8. Priority Stack
**Dimension:** Goal (P), Criteria (S)
**Question:** "If you could ship only ONE feature of this product and nothing else, which one ships? What makes that the non-negotiable?"
**Follow-up probes:**
- "Does that one feature work without any of the other features you described?"
- "Is there a feature that sounds important but is actually just nice-to-have?"
- "Would the product still be useful if the second-most-important feature was missing?"
**Quality indicators:** Answer picks one feature and defends it with a user-centric argument (not technical ease). Strong answers can describe a minimal viable product around just that feature.

---

## 2. Architecture Domain

### A1. Integration Surface
**Dimension:** Constraint (P), Context (S)
**Question:** "List every external system, API, or service this product needs to talk to. For each, is the connection read-only, write-only, or bidirectional?"
**Follow-up probes:**
- "What happens if that external service changes its API or goes offline?"
- "Do you have credentials or sandbox access for each of these services right now?"
- "Is any external service a single point of failure for the whole product?"
**Quality indicators:** Answer lists specific services (not "a database" but "PostgreSQL 16 on Supabase"). Strong answers include auth method (API key, OAuth) and failure behavior per integration. Weak answers are vague about which services.

### A2. Data Flow Shape
**Dimension:** Goal (P), Constraint (S)
**Question:** "Describe the primary data flow: where does data enter the system, where is it stored, and where does it exit? Think of it as a pipeline — what are the stages?"
**Follow-up probes:**
- "Is any stage asynchronous (batch, queue, eventual) or is everything synchronous?"
- "Where does data transformation happen — at ingestion, at storage, or at display?"
- "Is there a stage where data is validated, and what happens to invalid data?"
**Quality indicators:** Answer describes a clear pipeline with 3-5 named stages. Strong answers specify data format at each stage and indicate where validation/transformation occurs. Weak answers describe only UI-level flow without backend stages.

### A3. State Management
**Dimension:** Constraint (P), Criteria (S)
**Question:** "What is the source of truth for the system's state? Is there a single database, multiple stores, or does the UI hold state that the backend doesn't know about?"
**Follow-up probes:**
- "Can two users see different states for the same data at the same time? Is that acceptable?"
- "Is there optimistic UI (show changes before server confirms) or pessimistic (wait for server)?"
- "What is the expected latency between a state change and all users seeing it?"
**Quality indicators:** Answer specifies one primary source of truth and any caches or replicas. Strong answers address consistency model (eventual vs. strong) and conflict resolution. Weak answers say "the database" without specifying which one or how consistency works.

### A4. Scale Envelope
**Dimension:** Constraint (P), Criteria (S)
**Question:** "What are the realistic numbers? How many users, how much data, how many requests per second — not 'what's the dream' but 'what's the launch-day reality' and 'what's the year-one reality'?"
**Follow-up probes:**
- "Are these concurrent users or total registered users? The difference is 100x."
- "What is the largest single object (file, record, document) the system handles?"
- "At what scale would you need to fundamentally change the architecture?"
**Quality indicators:** Answer provides concrete numbers with a time horizon. Strong answers distinguish launch vs. growth vs. breaking-point thresholds. Weak answers say "millions" without defining what that means for the system.

### A5. Technology Constraints
**Dimension:** Constraint (P), Context (S)
**Question:** "Are there technologies that are mandated or forbidden? Not preferences — hard requirements from the team, the org, or the deployment environment."
**Follow-up probes:**
- "Is there an existing CI/CD pipeline, and does it constrain your language or framework choice?"
- "Are there budget constraints on infrastructure (e.g., must run on a single server, must use free-tier services)?"
- "Is there a mandated database, message queue, or deployment platform?"
**Quality indicators:** Answer distinguishes hard constraints (org mandate, existing infra) from soft preferences. Strong answers cite specific reasons (compliance, team skill set, existing investment). Weak answers list preferences as constraints.

### A6. Migration and Backward Compatibility
**Dimension:** Constraint (P), Context (S) -- Brownfield primary
**Question:** "What existing data, schemas, or APIs must this product be compatible with? Is there a migration path, or does this coexist with the old system?"
**Follow-up probes:**
- "Is there a cut-over date, or will old and new systems run in parallel indefinitely?"
- "Can the existing data format change, or is it frozen by other consumers?"
- "Are there existing integrations that depend on the current system's behavior?"
**Quality indicators:** Answer specifies compatibility requirements with concrete data types or API contracts. Strong answers describe a phased migration with rollback capability. Weak answers say "we'll migrate everything" without specifying how.

### A7. Deployment Topology
**Dimension:** Constraint (P), Criteria (S)
**Question:** "Where does this system run? Single server, containers, serverless, edge? And who operates it — the dev team, a platform team, or the user?"
**Follow-up probes:**
- "Is there a staging environment, and does it mirror production?"
- "What is the expected deployment frequency — daily, weekly, per-PR?"
- "Can you roll back a bad deployment in under 5 minutes?"
**Quality indicators:** Answer names a specific deployment target (AWS ECS, Vercel, bare metal) and an operator persona. Strong answers include rollback strategy and environment parity. Weak answers are vague ("the cloud").

### A8. Security Architecture
**Dimension:** Constraint (P), Criteria (S)
**Question:** "How does authentication and authorization work end-to-end? Not 'we use JWT' — I mean: where does the user enter credentials, where are they verified, and how does every subsequent request prove who they are?"
**Follow-up probes:**
- "Is there a single sign-on (SSO) requirement, or is local auth acceptable?"
- "What happens when a token expires mid-session?"
- "Are there different permission levels, and where are they enforced — API layer, middleware, or database?"
**Quality indicators:** Answer describes the full auth flow from credential entry to request authorization. Strong answers specify token storage, refresh strategy, and enforcement point. Weak answers mention "OAuth" without describing the flow.

---

## 3. UX Domain

### U1. Entry Point
**Dimension:** Goal (P), Criteria (S)
**Question:** "What is the very first screen or interaction a new user sees? Walk me through their first 30 seconds."
**Follow-up probes:**
- "Is there a sign-up flow, or do they land directly in the product?"
- "What does the user know before they arrive — did they read a landing page, get an invite, or stumble in?"
- "Is there an onboarding tutorial, and if so, can the user skip it?"
**Quality indicators:** Answer describes a specific first screen with a clear primary action. Strong answers account for different entry contexts (invite vs. organic). Weak answers describe the dashboard without explaining how users get there.

### U2. Interaction Model
**Dimension:** Goal (P), Constraint (S)
**Question:** "Is this a real-time collaborative tool (like Google Docs), a single-player tool that syncs (like Notion), or a request-response tool (like a form builder)? The interaction model changes everything."
**Follow-up probes:**
- "If two users edit the same thing simultaneously, what happens?"
- "Does the user need to explicitly save, or is everything auto-saved?"
- "Are there undo/redo capabilities, and how far back do they go?"
**Quality indicators:** Answer picks one model and defends it. Strong answers address conflict resolution and persistence model. Weak answers try to combine multiple models without specifying which takes precedence.

### U3. Information Architecture
**Dimension:** Goal (P), Criteria (S)
**Question:** "How many distinct views or pages does this product have? Name each one and its primary purpose. If you had to put them in a nav bar, what would the items be?"
**Follow-up probes:**
- "Is there a search function, and what can be searched?"
- "Are there nested views (project > task > subtask), and how deep does nesting go?"
- "Can the user customize the navigation or layout?"
**Quality indicators:** Answer names 3-7 top-level views with clear purposes. Strong answers show a hierarchy and indicate which views are primary (used daily) vs. secondary (used occasionally). Weak answers list too many views without prioritization.

### U4. Feedback and Status
**Dimension:** Criteria (P)
**Question:** "How does the user know the system is working? What visual or textual feedback appears for: loading state, success, partial failure, and total failure?"
**Follow-up probes:**
- "Are there progress indicators for long-running operations?"
- "Does the user get notifications (in-app, email, push), and for what events?"
- "Is there a history or audit log the user can review?"
**Quality indicators:** Answer addresses at least loading/success/failure with specific UI elements. Strong answers include notification preferences and a status dashboard. Weak answers say "toast notifications" without specifying what triggers them.

### U5. Accessibility and Inclusivity
**Dimension:** Constraint (P)
**Question:** "Are there accessibility requirements — screen reader support, keyboard-only navigation, specific color contrast, or compliance with WCAG 2.2?"
**Follow-up probes:**
- "Is the primary audience in a specific language, or must this support internationalization?"
- "Are there users with limited technical literacy who need simplified interfaces?"
- "Will this be used in low-bandwidth or offline environments?"
**Quality indicators:** Answer addresses at least one accessibility dimension concretely. Strong answers cite a specific WCAG level and target audience constraints. Weak answers say "we'll make it accessible" without specifying how.

### U6. Mobile and Responsive
**Dimension:** Constraint (P), Criteria (S)
**Question:** "Does this need to work on mobile devices? If yes, is it a responsive web app, a native app, or a PWA? And which mobile features does it actually need (camera, GPS, push notifications, offline)?"
**Follow-up probes:**
- "What is the smallest screen size you will support?"
- "Are there interactions that only make sense on mobile (swipe, long-press) or only on desktop (hover, right-click)?"
- "Does the mobile experience need feature parity with desktop?"
**Quality indicators:** Answer specifies the mobile strategy and minimum viable mobile features. Strong answers distinguish mobile-primary vs. mobile-acceptable interactions. Weak answers say "it should work everywhere" without specifying constraints.

### U7. Visual Design Direction
**Dimension:** Goal (P), Constraint (S)
**Question:** "Is there a design system, brand guidelines, or existing UI framework that constrains the visual design? Or is this greenfield design?"
**Follow-up probes:**
- "Are there reference products whose visual style you want to emulate or avoid?"
- "Is dark mode required, or is light-only acceptable?"
- "Are there any visual elements that are brand-mandated (logo, color palette, typography)?"
**Quality indicators:** Answer identifies constraints (existing design system) or gives clear direction (minimal, enterprise, playful). Strong answers cite specific frameworks or reference products. Weak answers say "modern and clean" which is not actionable.

---

## 4. Performance Domain

### P1. Latency Budget
**Dimension:** Constraint (P), Criteria (S)
**Question:** "What is the acceptable response time for the most critical user action? Not 'as fast as possible' — give me a number. Is 100ms fine? 500ms? 2 seconds?"
**Follow-up probes:**
- "Is that number measured from button click to screen update, or from API request to API response?"
- "Are there actions where a slower response is acceptable (background jobs, reports)?"
- "What should the user see while waiting — a spinner, skeleton screen, or placeholder?"
**Quality indicators:** Answer provides a concrete latency target (e.g., "under 200ms for search, under 2s for report generation"). Strong answers differentiate between interaction types. Weak answers say "fast" without numbers.

### P2. Data Volume Expectations
**Dimension:** Constraint (P)
**Question:** "What is the realistic data volume at launch and after one year? Count: number of records, total storage size, and typical query result set size."
**Follow-up probes:**
- "What is the largest single dataset the system will process at once?"
- "Is historical data retained indefinitely, or is there a retention policy?"
- "Will the data grow linearly, exponentially, or in bursts?"
**Quality indicators:** Answer provides specific numbers with a growth trajectory. Strong answers address retention policy and growth rate. Weak answers say "not that much" without quantifying.

### P3. Concurrency Model
**Dimension:** Constraint (P), Criteria (S)
**Question:** "How many concurrent users does the system need to handle? And more importantly, what are those users doing — reading, writing, or both?"
**Follow-up probes:**
- "Is there a peak usage pattern (e.g., Monday mornings, end of month)?"
- "Are there operations that must be serialized (two users cannot edit the same record), or is everything parallelizable?"
- "What is the ratio of reads to writes?"
**Quality indicators:** Answer specifies concurrent user count, read/write ratio, and peak patterns. Strong answers describe the contention model (what happens when two users want the same resource). Weak answers give only total users without concurrency profile.

### P4. Offline and Caching
**Dimension:** Constraint (P), Criteria (S)
**Question:** "Does any part of this system need to work offline? If yes, what specific features must work without connectivity, and what happens to changes made offline?"
**Follow-up probes:**
- "When connectivity returns, how are offline changes reconciled with server state?"
- "Is there a cache invalidation strategy, and what is the acceptable staleness window?"
- "Are there parts of the data that should be preloaded for offline use?"
**Quality indicators:** Answer specifies which features require offline support and the sync strategy. Strong answers describe conflict resolution and staleness bounds. Weak answers say "it should work offline" without specifying scope.

### P5. Background Processing
**Dimension:** Goal (P), Constraint (S)
**Question:** "Are there any long-running operations — reports, exports, batch imports, ML inference, email sends? How long do they take, and how does the user know they're in progress?"
**Follow-up probes:**
- "Can these operations be cancelled or paused midway?"
- "Should the user be notified when the operation completes, or do they check back?"
- "Are there rate limits on these operations (e.g., max 1 export per hour)?"
**Quality indicators:** Answer identifies specific long-running operations with estimated durations. Strong answers describe progress feedback, cancellation, and notification strategy. Weak answers list operations without considering user experience during the wait.

---

## 5. Security Domain

### S1. Authentication Model
**Dimension:** Constraint (P), Criteria (S)
**Question:** "How do users prove who they are? Walk me through the authentication flow from the moment a user opens the app for the first time to when they're fully authenticated."
**Follow-up probes:**
- "Is there multi-factor authentication, and is it required or optional?"
- "How long do sessions last, and what triggers re-authentication?"
- "Is there a 'remember me' option, and what does it change?"
**Quality indicators:** Answer describes a complete auth flow with specific mechanisms (password, SSO, passkey). Strong answers address session management, MFA policy, and token lifecycle. Weak answers say "standard login" without specifics.

### S2. Authorization Model
**Dimension:** Constraint (P), Criteria (S)
**Question:** "Once authenticated, what can each user role do? Describe the permission model — is it role-based (RBAC), attribute-based (ABAC), or something else?"
**Follow-up probes:**
- "Can permissions be customized per-user, or are they fixed per-role?"
- "Where are permissions checked — every API endpoint, middleware, or database level?"
- "Is there an admin override that bypasses all permissions?"
**Quality indicators:** Answer names the authorization model and lists role-permission mappings. Strong answers specify the enforcement layer and describe the admin override policy. Weak answers say "admin can do everything" without defining what "everything" includes.

### S3. Data Protection
**Dimension:** Constraint (P), Criteria (S)
**Question:** "What data in this system is sensitive? I mean data that, if leaked, would cause harm — PII, financial data, health records, API keys, business secrets. For each category, how is it protected?"
**Follow-up probes:**
- "Is sensitive data encrypted at rest and in transit? What encryption standard?"
- "Are there fields that should be masked in logs or admin views?"
- "Is there a data classification policy, and does the system enforce it?"
**Quality indicators:** Answer identifies specific sensitive data categories with concrete protection measures. Strong answers address encryption at rest/in transit, log masking, and data classification. Weak answers say "everything is encrypted" without specifying what and how.

### S4. Input Validation and Injection
**Dimension:** Criteria (P), Constraint (S)
**Question:** "Where does user input enter the system, and how is it validated? Think about text fields, file uploads, URL parameters, and API request bodies."
**Follow-up probes:**
- "Is there a centralized validation layer, or does each endpoint validate independently?"
- "What happens when input fails validation — reject, sanitize, or warn?"
- "Are there file upload restrictions (type, size, content scanning)?"
**Quality indicators:** Answer identifies all input vectors and validation strategy. Strong answers describe a centralized validation approach with specific rules per input type. Weak answers say "we validate everything" without specifying where or how.

### S5. Audit and Compliance
**Dimension:** Constraint (P), Criteria (S)
**Question:** "Are there regulatory or compliance requirements that affect how this system is built? GDPR, HIPAA, SOC 2, PCI-DSS, or anything else?"
**Follow-up probes:**
- "Must there be an audit log of who accessed what data and when?"
- "Are there data retention or deletion requirements (right to be forgotten)?"
- "Is there a data processing agreement required with any third-party service?"
**Quality indicators:** Answer names specific compliance frameworks (not just "we need to be compliant"). Strong answers describe required controls (audit log, data retention, deletion flow) tied to specific regulations. Weak answers are vague about compliance scope.

### S6. API Security
**Dimension:** Constraint (P), Criteria (S)
**Question:** "If this system exposes an API, how is it secured? Rate limiting, authentication, CORS, input size limits — what is the defense-in-depth strategy?"
**Follow-up probes:**
- "Is the API public, partner-only, or internal-only?"
- "Are there rate limits per user, per IP, or per API key?"
- "Is there an API versioning strategy, and how are breaking changes handled?"
**Quality indicators:** Answer describes layered API security (auth + rate limiting + input validation + CORS). Strong answers specify rate limit values, API versioning strategy, and public vs. internal distinction. Weak answers mention only API keys without addressing other layers.

### S7. Incident Response
**Dimension:** Criteria (P), Constraint (S)
**Question:** "If this system is compromised — data breach, unauthorized access, or service disruption — what is the expected response? Not 'we'll deal with it' — what specific mechanisms exist?"
**Follow-up probes:**
- "Is there an alerting system for suspicious activity?"
- "Can access be revoked immediately (kill switch for API keys, force logout)?"
- "Is there a backup and restore procedure, and what is the RTO/RPO?"
**Quality indicators:** Answer describes detection, response, and recovery mechanisms. Strong answers specify alerting thresholds, access revocation speed, and backup RTO/RPO. Weak answers say "we'll notify affected users" without specifying how detection happens.

---

## 6. Challenge Mode Questions (Cross-Domain)

These questions activate at specific round thresholds per the deep-interview protocol. They are cross-domain by design.

### C1. Contrarian Mode (Round 4+)
**Dimension:** Targets whichever dimension has the highest score (challenges the "settled" areas)
**Question:** "You've been confident about [settled assumption]. What if the opposite were true — would the whole design change, or would it survive?"
**Follow-up probes:**
- "Is this requirement driven by data, or is it an assumption you haven't tested?"
- "If this constraint disappeared tomorrow, what would you build differently?"
- "What is the weakest link in the chain of decisions we've made so far?"
**Quality indicators:** User either reinforces the assumption with evidence (validates) or recognizes it was untested (opens new clarity). Both outcomes are productive. A non-answer ("I guess it could be different") indicates the assumption needs investigation.

### C2. Simplifier Mode (Round 6+)
**Dimension:** Constraint (P) -- targets over-specified requirements
**Question:** "Looking at everything we've discussed, what is the simplest version of this that would still be valuable to someone? Not a compromise — a genuine minimal core."
**Follow-up probes:**
- "Which features did you describe because users need them, and which because they seemed expected?"
- "If you had to build this in one weekend, what would survive?"
- "Are there constraints that exist because of habit rather than necessity?"
**Quality indicators:** User can name a minimal version that still delivers the core value. Strong answers cut 50%+ of scope while preserving the product's reason for existing. Weak answers cut features but keep all constraints.

### C3. Ontologist Mode (Round 8+, ambiguity > 0.3)
**Dimension:** Goal (P) -- targets fundamental concept instability
**Question:** "We've been talking about this for {n} rounds and the entities keep shifting. What IS this product, fundamentally? Not what it does — what it IS."
**Follow-up probes:**
- "Looking at the entity list we've tracked, which one is the load-bearing concept?"
- "If you removed one entity and the product stopped making sense, which entity is that?"
- "Is this product a tool, a platform, a marketplace, or a service? Not 'all of the above' — pick one."
**Quality indicators:** User produces a single, stable core concept. Strong answers identify the atomic entity that everything else depends on. Weak answers continue to describe features rather than the essence.

---

## 7. Brownfield-Specific Questions (Context Dimension)

These questions target the Context Clarity dimension, which only applies to brownfield interviews.

### B1. Existing System Map
**Dimension:** Context (P)
**Question:** "I've explored the codebase and found [cited findings]. Are these the relevant parts of the system, or am I missing critical areas?"
**Follow-up probes:**
- "Are there undocumented parts of the system that only certain people understand?"
- "Is there technical debt in the areas we'd need to modify?"
- "Are there tests in place for the parts we'll be changing?"
**Quality indicators:** User confirms or corrects the codebase map. Strong answers add tribal knowledge that the codebase doesn't reveal. Weak answers say "that sounds right" without adding context.

### B2. Existing Constraints
**Dimension:** Context (P), Constraint (S)
**Question:** "What about the existing system must not change? Not 'should not' — MUST NOT. What are the immovable pieces?"
**Follow-up probes:**
- "Are there external consumers of the current API or data format?"
- "Are there shared databases or services that other teams depend on?"
- "Is there a deployment pipeline that constrains how changes are deployed?"
**Quality indicators:** User names specific immovable constraints with reasons (other teams depend on it, external API contract, compliance). Strong answers distinguish hard constraints from preferences. Weak answers are vague ("the backend should stay the same").

### B3. Migration Risk
**Dimension:** Context (P), Criteria (S)
**Question:** "What is the riskiest part of modifying the existing system? Where have past changes caused unexpected breakage?"
**Follow-up probes:**
- "Are there areas of the codebase that the team is afraid to touch?"
- "Is there a rollback strategy if the change causes problems?"
- "Are there integration tests that cover the areas we'll be modifying?"
**Quality indicators:** User identifies specific high-risk areas with historical context. Strong answers describe past incidents and mitigation strategies. Weak answers say "it should be fine" without acknowledging risk.

---

## Usage Notes for the Interview Agent

1. **Dimension-based selection:** When the weakest dimension is `Goal`, prioritize questions from sections R (Requirements) and U (UX). When `Constraint` is weakest, lean on A (Architecture), P (Performance), and S (Security). When `Criteria` is weakest, focus on the "Follow-up probes" and "Quality indicators" columns across all domains.

2. **Round-appropriate depth:** Early rounds (1-3) should use foundational questions (R1-R3, A1-A2, U1). Middle rounds (4-7) go deeper (R4-R8, A3-A8, S1-S4). Late rounds (8+) target edge cases and challenge modes (C1-C3, B1-B3, S5-S7).

3. **Brownfield bonus:** For brownfield interviews, always combine a domain question with a Context question. Example: ask A1 (Integration Surface) followed by B1 (Existing System Map) to cross-reference the user's answers against codebase evidence.

4. **Topology rotation:** When multiple components are active, do not repeat the same domain for consecutive rounds. If round N asked an Architecture question for component A, round N+1 should ask a UX or Security question for component B.

5. **Question adaptation:** These questions are templates, not scripts. Adapt wording to match the user's domain vocabulary. If the user calls it a "workspace" not a "project", use their term.
