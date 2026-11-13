# The Guru Method

How Guru turns "I want to learn X" into a short, intense course that you practise rather
than watch, and the research behind each design decision. The rules are enforced in code:
`core/src/prompts.rs` holds what Codex is told when it writes a course, `core/src/sim.rs`
holds what makes a simulation valid, and `ui/js/app.js` defines how you move through it.

## Fast by design

Most courses are slow for reasons that have nothing to do with learning: filler,
repetition, waiting for installs, hunting for a practice environment, getting stuck
without feedback. Guru removes those:

- **Scoped by the level bar.** You choose the destination (beginner → intermediate →
  advanced) and Guru sizes the course to it. Sizing is deliberately aggressive: material
  that is usually sold as "two weeks" is planned as a few focused hours. A single tool
  such as Grafana reaches intermediate in about 1.5–3 hours; a framework in about 2.5–4;
  a broad discipline such as DevOps in about 4–7.
- **Pareto content.** Each course teaches the 20% of concepts, commands, workflows and
  failure modes that cover most real use, ranked by real-world value. What is out of
  scope is written down (Kaufman, 2013).
- **A target performance.** Every course states what you will be able to *do* at the
  end. Simulations check it, and mastery per skill is computed from your answers and
  actions, not from time spent.
- **Zero setup.** Practice happens in simulations inside the app, so no time goes on
  installing, configuring or paying for tools.

## A module: Lesson → Quiz → Simulation

| Step | What happens | Why |
|---|---|---|
| **Lesson** (reading-first) | Why it matters → the mental model → a fully worked example with *Pause and predict* prompts → the 20% you'll use constantly → how it breaks (real failures with sources) → what not to confuse it with → cheat sheet | Studying worked examples beats unguided problem solving for novices (*worked-example effect*; Sweller & Cooper, 1985). Self-explanation prompts deepen understanding (Chi et al., 1994), and committing to a prediction first improves memory (*generation effect*; Slamecka & Graf, 1978). Contrasting similar ideas sharpens discrimination (Kornell & Bjork, 2008). |
| **Quiz** | Recall and application questions, with misconceptions as distractors and an explanation after every answer | Retrieval practice is one of the most effective techniques known (Roediger & Karpicke, 2006; Karpicke & Blunt, 2011; rated high utility by Dunlosky et al., 2013). Immediate, specific feedback corrects errors before they set (Hattie & Timperley, 2007). |
| **Simulation** | A realistic world (an incident, a tool, a build, a conversation) with objectives, graduated hints and an expert debrief | Simulation-based learning has large positive effects on complex skills (meta-analysis: Chernikova et al., 2020). Deliberate practice with feedback at the edge of ability builds skill (Ericsson et al., 1993). Some struggle before getting help improves transfer (*productive failure*; Kapur, 2008). Hints fade support gradually (Renkl et al., 2002). |
| **Flashcards** (across the course) | Cards from every module, interleaved, on an expanding schedule | Spacing beats cramming (Cepeda et al., 2006). Interleaving improves discrimination (Rohrer & Taylor, 2007). |
| **Capstone** | One multi-step scenario that integrates the whole course | Whole-task practice connects the parts (van Merriënboer & Kirschner, *Ten Steps to Complex Learning*). |

## Why simulations, and what makes a good one

The gap Guru closes is access: many people never practise a tool because it needs an
account, a licence, a cluster or a machine they don't have. A simulation recreates the
*decisions and feedback* of the real thing, which is what practice needs:

- **Realistic signal in realistic noise.** Logs contain normal background activity, the
  real clue, and a red herring or two, so you find the answer instead of guessing it.
- **The tool's real vocabulary.** Query editors accept real PromQL or SQL and return the
  tool's real error messages, so the habits transfer.
- **Finding and fixing.** Objectives check both what you understood (the root cause, the
  attacker's IP) and what you did (process stopped, persistence removed, alert created).
- **Real failure modes.** Scenarios are seeded from the most up-voted Stack Overflow /
  Stack Exchange questions for the topic, which is a good proxy for the problems most
  practitioners actually hit.
- **Always playable.** A validator rejects simulations whose objectives can't be
  completed, and Codex fixes them before the course is marked ready.

## Spacing across days

Courses are split into modules of about 45 minutes, so a few hours fits naturally into a
few days. That spacing is a feature: distributed practice beats massed practice (Cepeda
et al., 2006), and sleep consolidates what you learned that day (Walker & Stickgold,
2004). Flashcards follow SM-2 (Wozniak, 1990) with a compressed early schedule (1 day, 3
days, then expanding), so cards are revisited inside the course and keep coming back
afterwards.

## Why AI, and only up front

One-to-one tutoring with mastery learning produces large gains (Bloom, 1984). Good
step-based tutoring systems come close to human tutors (VanLehn, 2011). Guru uses AI to
*author* that kind of material (explanations, feedback for every wrong answer, scenarios
that respond to your actions) once, at download time. Studying then needs no AI. That
keeps it fast, private, cheap, and usable offline. Optional "Ask AI" buttons exist for the
rare question the course did not anticipate.

## References

- Bloom, B. S. (1984). The 2 sigma problem. *Educational Researcher, 13*(6), 4-16.
- Cepeda, N. J., Pashler, H., Vul, E., Wixted, J. T., & Rohrer, D. (2006). Distributed practice in verbal recall tasks: A review and quantitative synthesis. *Psychological Bulletin, 132*(3), 354-380.
- Chernikova, O., Heitzmann, N., Stadler, M., Holzberger, D., Seidel, T., & Fischer, F. (2020). Simulation-based learning in higher education: A meta-analysis. *Review of Educational Research, 90*(4), 499-541.
- Chi, M. T. H., de Leeuw, N., Chiu, M.-H., & LaVancher, C. (1994). Eliciting self-explanations improves understanding. *Cognitive Science, 18*(3), 439-477.
- Dunlosky, J., Rawson, K. A., Marsh, E. J., Nathan, M. J., & Willingham, D. T. (2013). Improving students' learning with effective learning techniques. *Psychological Science in the Public Interest, 14*(1), 4-58.
- Ericsson, K. A., Krampe, R. T., & Tesch-Römer, C. (1993). The role of deliberate practice in the acquisition of expert performance. *Psychological Review, 100*(3), 363-406.
- Hattie, J., & Timperley, H. (2007). The power of feedback. *Review of Educational Research, 77*(1), 81-112.
- Kapur, M. (2008). Productive failure. *Cognition and Instruction, 26*(3), 379-424.
- Karpicke, J. D., & Blunt, J. R. (2011). Retrieval practice produces more learning than elaborative studying with concept mapping. *Science, 331*(6018), 772-775.
- Kaufman, J. (2013). *The First 20 Hours: How to Learn Anything... Fast!* Portfolio/Penguin.
- Kornell, N., & Bjork, R. A. (2008). Learning concepts and categories: Is spacing the "enemy of induction"? *Psychological Science, 19*(6), 585-592.
- Renkl, A., Atkinson, R. K., Maier, U. H., & Staley, R. (2002). From example study to problem solving: Smooth transitions help learning. *Journal of Experimental Education, 70*(4), 293-315.
- Roediger, H. L., & Karpicke, J. D. (2006). Test-enhanced learning: Taking memory tests improves long-term retention. *Psychological Science, 17*(3), 249-255.
- Rohrer, D., & Taylor, K. (2007). The shuffling of mathematics problems improves learning. *Instructional Science, 35*(6), 481-498.
- Slamecka, N. J., & Graf, P. (1978). The generation effect: Delineation of a phenomenon. *Journal of Experimental Psychology: Human Learning and Memory, 4*(6), 592-604.
- Sweller, J., & Cooper, G. A. (1985). The use of worked examples as a substitute for problem solving in learning algebra. *Cognition and Instruction, 2*(1), 59-89.
- van Merriënboer, J. J. G., & Kirschner, P. A. (2017). *Ten Steps to Complex Learning* (3rd ed.). Routledge.
- VanLehn, K. (2011). The relative effectiveness of human tutoring, intelligent tutoring systems, and other tutoring systems. *Educational Psychologist, 46*(4), 197-221.
- Walker, M. P., & Stickgold, R. (2004). Sleep-dependent learning and memory consolidation. *Neuron, 44*(1), 121-133.
- Wozniak, P. A. (1990). *Optimization of learning* (SuperMemo SM-2 algorithm). Master's thesis, University of Technology in Poznań.
