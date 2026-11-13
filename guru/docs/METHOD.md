# The Guru Method

How Guru gets a learner to working competence in about seven focused hours, and the
research behind each design decision. Every rule here is enforced in code, mostly in
`src/prompts.rs` (what Codex is told to produce) and `src/session.rs` (how an hour runs).

## What "7 hours" honestly buys

Seven hours will not make anyone a seasoned professional. Expertise in a field comes from
years of varied, deliberate practice (Ericsson et al., 1993). What seven *well-designed*
hours can buy is **working, intermediate-level competence in a deliberately scoped slice**:
you can do the common real tasks on your own, you recognise and fix the most frequent
failures, and you know where the edges of your knowledge are. Kaufman (2013) argues that
~20 hours of smart practice gets most people from "grossly incompetent" to "reasonably
good". Guru compresses that further by doing the expensive preparation for you:
deconstructing the skill, building practice environments and mining real problems.

So Guru never promises "everything about Spring Boot". Before hour 1 it commits to a
**target performance** (for example: *"build, test and containerise a CRUD REST API with
validation and JPA, and diagnose the 10 most common startup/configuration failures"*),
writes down what is **out of scope**, and measures you against that target every hour.
At the end, `guru report` says plainly whether you met it and plans the next 7 hours.

## The course: deconstruct, then calibrate

1. **Goal → target performance.** You say what you want to *do*. Guru turns it into one
   checkable performance and a capstone project that grows a slice every hour.
   *Whole-task learning*: complex skills are learned best through realistic whole tasks
   with support that fades over time, not as isolated parts (van Merriënboer & Kirschner,
   *Ten Steps to Complex Learning*; the 4C/ID model).
2. **Skill deconstruction, Pareto-ranked.** 8-16 sub-skills, each with a weight for how
   much real-world performance it buys, ordered by dependency. The highest-weight skills
   come first, so stopping early still leaves you with the most useful 20%
   (Kaufman, 2013).
3. **Pretest.** Five questions *before* any teaching. Trying to answer, even when wrong,
   improves later learning of that material (the *pretesting effect*; Richland, Kornell &
   Kao, 2009). It also tells Guru what to skip: learners who already know more need less
   scaffolding and worked examples (*expertise reversal*; Kalyuga et al., 2003).
4. **Real-world issue bank.** Guru pulls the most up-voted questions for the topic from the
   Stack Exchange API and has Codex turn them, together with official docs and issue
   trackers, into a bank of symptom → root cause → fix entries with source links. Votes
   are a decent proxy for *how many practitioners hit this*. These become break-fix
   tasks and incident drills.

## The hour: the Guru Loop (60 minutes, scaled if you change the length)

| Block | Time | What happens | Why |
|---|---|---|---|
| **Recall** | 5 min | Due flashcards from earlier hours, interleaved across skills, self-graded | Retrieval practice is among the most effective techniques known (Roediger & Karpicke, 2006; Karpicke & Blunt, 2011). Rated "high utility" in the major review by Dunlosky et al. (2013). Interleaving improves discrimination between similar ideas (Kornell & Bjork, 2008; Rohrer & Taylor, 2007). |
| **Read** | 15 min | Predict first, then a 900-1800 word lesson: why it matters → mental model → worked example with "pause and predict" prompts → the 20% → how it breaks → contrast → cheat sheet | Generating an answer first improves memory (*generation effect*; Slamecka & Graf, 1978). Studying worked examples beats unguided problem solving for novices (*worked-example effect*; Sweller & Cooper, 1985). Self-explanation prompts deepen understanding (Chi et al., 1994). Reading-first because you asked for it, and because text is fast to skim, search and re-read. |
| **Practice** | 25 min | A lab sized to your machine: completion problem → independent task → break-fix from the real issue bank → capstone slice. Three-level hint ladder, AI hints that read your actual files, live tutor | Fading from worked examples to independent problems (Renkl et al., 2002). Deliberate practice at the edge of ability with immediate feedback (Ericsson et al., 1993; Hattie & Timperley, 2007). Struggling a little before getting help helps transfer (*productive failure*; Kapur, 2008). |
| **Prove** | 10 min | Lab check, exit quiz (recall + apply + interleaved earlier skills) with a confidence rating for each answer, then "explain it simply" | Testing is itself learning, not only measurement. Confidence ratings expose overconfidence, which otherwise makes learners stop studying too early (Dunlosky & Rawson, 2012). Explaining to a novice reveals gaps (Feynman technique; self-explanation). |
| **Reflect** | 5 min | Scores, calibration, mastery gate, new cards for exactly what you missed, cleanup, what's next | *Mastery learning*: do not build on weak foundations; repair first (Bloom, 1984). Errors become targeted retrieval practice. |

### Mastery gate and adaptivity

Each hour is scored (quiz 50%, lab 35%, explanation 15%). Below 80%, the next hour **opens
with remediation** of the weak skills before new material. Mastery per skill is an
evidence-weighted running estimate (60% new evidence, 40% history). Misconceptions are
recorded and explicitly repaired. A coach note carries pacing advice forward. Lessons and
labs are generated **just in time** from this learner model, which is why Guru is not a
static roadmap you get lost in: hour 4 is written knowing how hours 1-3 actually went.

### Spacing across the week

One hour a day is not a compromise; it is better than seven hours in one day.
Distributed practice reliably beats massed practice (Cepeda et al., 2006), and sleep
consolidates what was learned that day (Walker & Stickgold, 2004). Cards follow SM-2
(Wozniak, 1990) with a compressed second interval (1 day, then 3, then ×ease). A card
learned in hour 1 is therefore retrieved on day 2 and again around day 5, inside the
course, and the intervals keep growing afterwards for long-term retention. Guru warns
(but allows) when you do more than two hours in a day.

### Why AI tutoring, and its limits

Bloom (1984) reported that one-to-one tutoring with mastery learning raised outcomes by
about two standard deviations. Later meta-analysis puts human tutoring nearer 0.8 SD and
good step-based tutoring systems close to that (VanLehn, 2011). Guru aims for the
ingredients behind those results: immediate feedback on each step, mastery gating, and
adaptive problem selection. It is not magic. Your effort during retrieval and practice
is the active ingredient, so Guru is designed to make you produce answers, not watch them.

## Practice arenas: not only code

The planner picks the arena that gives the most realistic practice for the topic:

| Arena | For | Practice looks like |
|---|---|---|
| `code_lab` | languages, frameworks, libraries | a real project with TODOs and automated checks |
| `ops_lab` | DevOps, infrastructure, databases, networking | broken systems with real faults injected; diagnose from logs and symptoms; simulated when the machine is small |
| `roleplay` | spoken languages, interviews, negotiation, sales | scenarios played live with the tutor as counterpart, corrected after each turn; frequency-first vocabulary and pattern drills |
| `problem_set` | maths, physics, finance, statistics | worked example → completion → independent → transfer problems |
| `drill_protocol` | instruments, drawing, typing, sports | timed drills with objective rubrics and a practice log |
| `case_study` | management, law, medicine, strategy | realistic case files requiring a decision; expert-reasoning debrief |

For languages, frequency matters a lot. A few thousand high-frequency word families cover
most running words in everyday text and speech (Nation, 2006), so Guru teaches the most
frequent words and sentence patterns first and makes you *produce* them.

## Respecting the machine

"Do not be Chrome." Guru measures the machine before every session and derives a hard
**lab budget**: at most half of the free RAM, and never more than 40% of total RAM
(cgroup limits respected), a disk cap, and CPU limits. It also decides whether containers
or a single-node local cluster are allowed. Every environment-building prompt carries
this budget. Labs must ship a teardown script, and Guru offers to stop everything at the
end of each hour. Guru itself is a ~1.2 MB binary using ~3 MB of RAM while idle. It runs
no daemon and spends memory only while Codex is working.

## References

- Bjork, R. A., & Bjork, E. L. (2011). Making things hard on yourself, but in a good way: Creating desirable difficulties to enhance learning. In *Psychology and the Real World*.
- Bloom, B. S. (1984). The 2 sigma problem. *Educational Researcher, 13*(6), 4-16.
- Cepeda, N. J., Pashler, H., Vul, E., Wixted, J. T., & Rohrer, D. (2006). Distributed practice in verbal recall tasks: A review and quantitative synthesis. *Psychological Bulletin, 132*(3), 354-380.
- Chi, M. T. H., de Leeuw, N., Chiu, M.-H., & LaVancher, C. (1994). Eliciting self-explanations improves understanding. *Cognitive Science, 18*(3), 439-477.
- Dunlosky, J., & Rawson, K. A. (2012). Overconfidence produces underachievement. *Learning and Instruction, 22*(4), 271-280.
- Dunlosky, J., Rawson, K. A., Marsh, E. J., Nathan, M. J., & Willingham, D. T. (2013). Improving students' learning with effective learning techniques. *Psychological Science in the Public Interest, 14*(1), 4-58.
- Ericsson, K. A., Krampe, R. T., & Tesch-Römer, C. (1993). The role of deliberate practice in the acquisition of expert performance. *Psychological Review, 100*(3), 363-406.
- Hattie, J., & Timperley, H. (2007). The power of feedback. *Review of Educational Research, 77*(1), 81-112.
- Kalyuga, S., Ayres, P., Chandler, P., & Sweller, J. (2003). The expertise reversal effect. *Educational Psychologist, 38*(1), 23-31.
- Kapur, M. (2008). Productive failure. *Cognition and Instruction, 26*(3), 379-424.
- Karpicke, J. D., & Blunt, J. R. (2011). Retrieval practice produces more learning than elaborative studying with concept mapping. *Science, 331*(6018), 772-775.
- Kaufman, J. (2013). *The First 20 Hours: How to Learn Anything... Fast!* Portfolio/Penguin.
- Kornell, N., & Bjork, R. A. (2008). Learning concepts and categories: Is spacing the "enemy of induction"? *Psychological Science, 19*(6), 585-592.
- Nation, I. S. P. (2006). How large a vocabulary is needed for reading and listening? *Canadian Modern Language Review, 63*(1), 59-82.
- Renkl, A., Atkinson, R. K., Maier, U. H., & Staley, R. (2002). From example study to problem solving: Smooth transitions help learning. *Journal of Experimental Education, 70*(4), 293-315.
- Richland, L. E., Kornell, N., & Kao, L. S. (2009). The pretesting effect: Do unsuccessful retrieval attempts enhance learning? *Journal of Experimental Psychology: Applied, 15*(3), 243-257.
- Roediger, H. L., & Karpicke, J. D. (2006). Test-enhanced learning: Taking memory tests improves long-term retention. *Psychological Science, 17*(3), 249-255.
- Rohrer, D., & Taylor, K. (2007). The shuffling of mathematics problems improves learning. *Instructional Science, 35*(6), 481-498.
- Slamecka, N. J., & Graf, P. (1978). The generation effect: Delineation of a phenomenon. *Journal of Experimental Psychology: Human Learning and Memory, 4*(6), 592-604.
- Sweller, J., & Cooper, G. A. (1985). The use of worked examples as a substitute for problem solving in learning algebra. *Cognition and Instruction, 2*(1), 59-89.
- van Merriënboer, J. J. G., & Kirschner, P. A. (2017). *Ten Steps to Complex Learning* (3rd ed.). Routledge.
- VanLehn, K. (2011). The relative effectiveness of human tutoring, intelligent tutoring systems, and other tutoring systems. *Educational Psychologist, 46*(4), 197-221.
- Walker, M. P., & Stickgold, R. (2004). Sleep-dependent learning and memory consolidation. *Neuron, 44*(1), 121-133.
- Wozniak, P. A. (1990). *Optimization of learning* (SuperMemo SM-2 algorithm). Master's thesis, University of Technology in Poznań.
