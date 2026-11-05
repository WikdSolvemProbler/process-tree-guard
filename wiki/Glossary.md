# Glossary
<!-- SPDX-License-Identifier: Apache-2.0 -->
<!-- Copyright 2026 Peter Shiller -->

This glossary is the public face of the PTG-STE-1 dictionary for process-tree-guard. Every product term stands here with its one approved word and its one meaning. The machine source stands in the wiki set at `wiki/dictionary.tsv`.

## Contents

- [The local dictionary](#the-local-dictionary)
- [Terms](#terms)
  - [Controlled terms](#controlled-terms)
  - [Approved words](#approved-words)
- [Style rules](#style-rules)
- [Link colors](#link-colors)

## The local dictionary

PTG-STE-1 is the local Simplified Technical English word list and rule subset of this project, version 1. PTG-STE-1 is not ASD-STE100. PTG-STE-1 does not claim conformance to ASD-STE100. PTG-STE-1 follows the ASD-STE approach. That approach uses a controlled vocabulary. That approach uses one word for one meaning. That approach uses short sentences. That approach uses one instruction per sentence. That approach uses active voice.

The wiki file `wiki/dictionary.tsv` is the one machine source. This page and that file hold the same content. A difference between this page and that file is a defect.

The exception list in this section is the only exception to the approved words rule. A proper noun and a product name are not prose words. A code span, a file path, a vendor string, a flag spelling, and an exit code are artefacts, not prose. An approved phrase in the dictionary is one term. Numbers and units are not prose words.

[↑ Back to Contents](#contents)

## Terms

Each row gives one approved word and its one meaning. Each row gives the words that a writer does not use for that meaning.

### Controlled terms

| Approved word | One meaning | Do not use |
|---|---|---|
| adapter | The code in plugins/ that speaks one vendor protocol. | plugin; handler |
| anchor | A fixed point that a criterion serves and that a reader reads the criterion by. | reference point |
| backend | The declaration module and mechanisms for one operating system. | port; platform |
| battery | The suite's five decision cases, each ending in PASS or FAIL. | test set |
| campaign | One full configured measurement run of the suite. | run (for this); session |
| cap | The largest value that a rule allows. | limit; ceiling |
| carriage | A wrapper inside a value from the construction of that value. | storage |
| census | The counting that tests reach, file by file, channel by channel. | audit; profile |
| chase | The bounded following of a name through aliases and re-exports. | resolution (for this computation) |
| check | A hook kind that reads a payload and answers allow or deny. | kind (alone, for this) |
| checksum | A SHA-256 value that a user compares against a file before first use. | hash (alone, for this value) |
| command | The program and its arguments that a run executes. | job; task; workload |
| context | A hook kind that prints the installed policy and reads no payload. | kind (alone, for this) |
| counted-ops census | The counted list of named external calls on a run path. | census (alone, for this list) |
| deadline | The fixed future time at which a run ends. | timeout (as the concept noun); limit; expiry |
| declaration | A name that the platform module states for a platform mechanism or its data. | definition (for this) |
| demotion | Removal of a document into history, where it stays readable. | deletion (for this act) |
| end | To stop a process so that the process cannot run again, by the close of the job. | kill; terminate; stop (for this act) |
| envelope | A command line that already names the guard in the valid form. | wrapper (for this form) |
| family | The set of vendors or tests that share one decision computation. | group; class (for this set) |
| fixture | Data that a test reads as given, whose exact state is part of the test condition. | sample (for this) |
| floor | The time that two minimal tools agree on under one harness on one machine. | baseline (for this value) |
| guard | The program and library as a whole, the thing that keeps the promise. | tool; utility; app; wrapper (for the product) |
| hold | An approved extension of the deadline of a run, recorded in the log. | extension (for one granted instance) |
| host | A program that calls the guard through a hook. | vendor (for a program); client |
| job handle | The Windows handle that names a job object in a call. | — |
| job object | The Windows object that holds the processes of a run. | group; container (for this mechanism) |
| keeper | A named check, at a named step, whose failure blocks the claim that the check keeps. | review (alone, for this) |
| kill on close | The job object flag that ends every process in the job when the last job handle closes. | — |
| manifest | The one JSON file that names judged homes, tools/layout.json. | config; index (for this file) |
| mint | The closed set of names from which reach is judged. | source set |
| named place | A folder that holds only a README that states a tier and a revival condition. | stub; placeholder (for this) |
| owner | A process whose exit ends a run. | parent (for this role) |
| payload | The data that a host sends to a hook on standard input. | input; request; body (for this) |
| payload step | A step on a Result or Option value that passes the held value onward. | — |
| pin | A recorded SHA-256 value, or a recorded set, that a supplied artefact must match exactly. | lock; freeze (for this) |
| pin-match | The check that the tree matches a recorded pin. | verification (alone, for this check) |
| platform mechanism | A capability that only one operating system provides, in the terms of that system. | feature (for this) |
| policy file | POLICY.md as the program reads it at run time. | config (for this file) |
| process tree | A process and every process that starts from it, at any depth. | process group; children (for the whole set) |
| professor test | The shape test of anchor A1: a professor, shown the shape alone, can call that shape perfect. | — |
| pure file | A production file with a census count of zero on every channel. | clean file |
| pure set | The set of pure files, pinned by name, with its one pinned exception inside the definition. | — |
| reach | The fact that code uses a declaration, or uses a thing that itself uses a declaration in one step. | dependence (for this fact) |
| refusal | A named, recorded stop by a check that cannot prove its claim. | error; failure (for this act) |
| revival | The named event that makes a held item live: a named place becomes a backend, a non-execution runs, or the full census computation is restored from history and run. | TODO; future work (for this) |
| rewrite | A hook kind that reads a payload and answers with a rewritten command or a denial. | kind (alone, for this) |
| ring | The closed list of payload steps that the census reads, with its named exclusions. | list (alone, for this) |
| role | One of the four program forms: full, deadline, gate, or contain. | mode (for a program form) |
| run | One guarded execution of one command. | job; session; invocation (for a run) |
| selector | The one function that chooses among a closed set of texts by role. | dispatcher; picker (for this) |
| staging | The build step that copies verified binaries into the suite's build folder. | copy step |
| standing loss | A campaign loss that stands in the report until its cause is repaired. | failure (for this recorded loss) |
| suite | The test and benchmark package that ships at suite/. | test pack |
| sys | The one folder that holds the backends. | platform folder |
| tier | The evidence level of a claim: proven, type-checked only, or theory. | level; status (for this) |
| unevaluated | A dimension state: not exercised, therefore not passed and not failed. | skipped; unknown (for this state) |
| vendor | One hook protocol, named by its vendor string. | host (for a protocol); format (for the protocol itself) |
| verdict | The allow-or-deny answer that a hook gives. | decision (for the hook answer) |
| wrapper | A function or a method outside the backend module that itself reaches a platform mechanism. | helper (for this) |
| body | A body of code: the statements inside one function or module. | payload (for this) |
| client | The client part of an extendable run: the code that asks the server for a hold. | host (for this part) |
| container | A value that holds another value (a Result or an Option), or a machine boundary that holds processes as a class. | job object (for this) |
| extension | The mechanism that grants holds; one granted instance is a hold. | hold (for the mechanism) |
| group | A process group: the Unix mechanism, named only as a contrast to a process tree or a job object. | process tree; job object (for this) |
| input | The standard input stream as a channel name. | payload (for this) |
| level | A depth or rank in a structure, such as a heading level. | tier (for this) |
| limit | A stated boundary of what the guard guarantees or of what a rule states. | deadline; cap (for this) |
| mode | A form of operation inside a role, such as automatic mode. | role (for this) |
| parent | A process or file that another process or file directly comes from, as a fact of birth or structure. | owner (for this fact) |
| platform | An operating system environment as a class, such as a Windows platform. | backend (for this) |
| plugin | The whole four-part artefact of one integration: adapter, wiring, examples, and tests. | adapter (for the whole artefact) |
| request | A message that asks the guard or the extend server for a hold. | payload (for this) |
| session | A signed-in user's interactive logon period on a machine, or a host's working period. | run (for this) |
| status | The recorded state of a run, a campaign, or a check. | tier (for this) |
| task | A unit of work that a system or a host names, such as a scheduled task or Task Manager's list. | command (for this) |
| tool | A named function that a host offers, such as a Bash tool; never a name for the guard. | guard (for this) |
| workload | A Visual Studio component set that an installer selects, such as the C++ workload. | command (for this) |
| process group | The Unix mechanism that holds a process and the processes it starts as one group, named only as a contrast to a process tree or a job object. | process tree; job object (for this) |
| helper | A small unit of support code in the tests or the suite. | wrapper (for this) |
| launcher | The process that starts a run directly. | — |
| token | A system object that states the identity and the rights of a process. | credential (for this object) |
| arm | One side of a campaign: one tool under measurement, with its stated configuration. | — |
| free | With a census count of zero, in the phrase reach-free. | — |

### Approved words

| Approved word | One meaning | Do not use |
|---|---|---|
| a | One, any single. | — |
| aarch | The processor architecture name in artefact text. | — |
| able | In a state that permits a stated act. | — |
| about | Concerning a subject. | — |
| above | Higher than, or earlier in a text. | — |
| absence | The state of not being present. | — |
| absolute | Not measured against another value. | — |
| accept | To take a thing as valid. | — |
| acceptance | The act of taking a thing as given, without a check. | — |
| accepted | Taken as valid. | — |
| access | The right or the means to read or to change a stated thing. | — |
| accident | An event that happens without a stated plan. | — |
| account | A user's identity record on a machine or service. | — |
| across | From one side to the other of. | — |
| act | A thing done. | — |
| action | A step that a program or a person does. | — |
| activated | Made active. | — |
| active | In operation now. | — |
| ad | In artefact names: a directory service name part. | — |
| adapt | To change a thing so that it fits a new use. | — |
| add | To put one thing with another, or to increase a count. | — |
| address | A value that names a place in memory. | — |
| adjust | To change a value by a small amount to reach a target. | — |
| administrator | A user or role with full control of a machine. | — |
| adoption | The act of taking a stated thing into use. | — |
| advance | Before the stated time or step. | — |
| adversarial | Done by a hostile party that tries to break a rule. | — |
| after | Later than. | — |
| afterwards | At a later time. | — |
| again | One more time. | — |
| against | In opposition to, or in contact with. | — |
| agent | An automated program that acts for a person. | — |
| agree | To have the same answer or value. | — |
| agreement | A state in which two computations or parties give the same answer. | — |
| ahead | In front, or in the future. | — |
| alias | Another name for the same thing. | — |
| alike | In the same way, for both of two named things. | — |
| alive | Living, or still running. | — |
| all | Every one of a set. | — |
| allocations | Amounts of a resource that a system gives out. | — |
| allow | To permit an action. | — |
| alone | By itself, with nothing else. | — |
| already | Before the time in question. | — |
| also | In addition. | — |
| alternative | Another possible choice. | — |
| always | At every time. | — |
| among | Inside a set, taken as a group. | — |
| ampersand | The & character. | — |
| an | One, any single, before a vowel sound. | — |
| ancestor | A process or item that an item comes from, at any distance. | — |
| and | Joins two words or clauses as one set. | — |
| another | One more, or a different one. | — |
| answer | A reply that a check or a person gives. | — |
| anti | Prefix form in artefact words: against. | — |
| antigravity | Google Antigravity, a host product. | — |
| any | One or more, without a stated limit. | — |
| anything | Any thing at all. | — |
| apache | Apache, in the licence name Apache-2.0. | — |
| apart | Away from each other, as separate things. | — |
| api | API, the interface that one program offers to another, in artefact text. | — |
| appear | To become visible or to seem to be. | — |
| append | To add a thing at the end of a file or a record. | — |
| applicability | The state of applying to a stated case. | — |
| applicable | Applying to the stated case. | — |
| applied | Put into effect for a stated case. | — |
| apply | To put a rule or a value into effect. | — |
| approach | A stated way of reaching a result. | — |
| approval | Formal permission for a stated act. | — |
| approve | To give formal permission. | — |
| approximate | Near a stated value, but not exact. | — |
| architecture | The stated arrangement of a system's parts. | — |
| are | Plural present form of be. | — |
| area | A part of a surface, a subject, or a record. | — |
| argue | To give reasons for or against a claim. | — |
| argument | A set of reasons for a claim, or a value given to a command. | — |
| around | On all sides of, or near a value. | — |
| arrive | To reach a stated place or state. | — |
| arrives | Reaches a place or a state. | — |
| artefact | A file or object that a process produces. | — |
| as | In the role of, or at the same time. | — |
| asd | ASD, in the name ASD-STE100. | — |
| ask | To put a question, or to make a request by a defined call. | — |
| assert | To state a fact that a test then holds. | — |
| assertion | A statement that a test holds as true. | — |
| assignment | The act of placing a thing into a stated set at a stated time. | — |
| at | At a place, a time, or a value. | — |
| attempt | An act that tries to reach a result. | — |
| attribute | A named property in a list that a call gives to a system. | — |
| audience | The set of parties that read a stated thing. | — |
| audit | An examination of records against a rule. | — |
| auth | The auth folder name in the tree. | — |
| author | A person who writes a stated text or computation. | — |
| authoritative | Carrying the stated right to give the final answer. | — |
| authority | The right to give the final answer on a stated question. | — |
| auto | Short form in artefact names for the automatic role. | — |
| automatic | Done by a rule without a person choosing each time. | manual (for this) |
| availability | The state of being present and ready for use. | — |
| available | Present and ready for use. | — |
| away | To a distance, or absent. | — |
| axes | Plural of axis. | — |
| axis | A line of organization along which items are placed. | — |
| azure | Microsoft Azure, a service name in examples. | — |
| back | To an earlier place, or the rear part. | — |
| background | Work that runs without a screen or a person's attention. | — |
| backlog | A set of records that waits to be written or read. | — |
| backstop | A second means that holds a result when the first means fails. | — |
| banned | Not permitted. | — |
| bar | To forbid a stated act or claim. | — |
| bare | With nothing added. | — |
| base | The form from which other forms come. | — |
| bash | Bash, a shell program. | — |
| basic | Plain and necessary, with nothing extra. | — |
| be | To exist, or to have a stated quality. | — |
| bear | To carry a stated load. | — |
| because | For the reason that. | — |
| become | To come to be in a stated state. | — |
| been | Past participle of be. | — |
| before | Earlier than. | — |
| begins | Starts. | — |
| behaviour | The way a program acts under stated conditions. | behavior (spelling variant) |
| behind | At the back of. | — |
| believe | To take a stated fact as true. | — |
| belong | To be in the correct place or set. | — |
| below | Lower than, or later in a text. | — |
| benchmark | A measurement run that compares speed or cost. | — |
| beside | At the side of. | — |
| between | In the space that separates two things. | — |
| bin | The folder that holds built programs, in artefact names. | — |
| binaries | Built program files. | — |
| binary | A built program file, in artefact use. | — |
| bind | To tie a name or a shape to a stated thing. | — |
| birth | The coming into being of a thing. | — |
| bit | A small piece, or the smallest unit of data. | — |
| blame | To hold a party at fault for a stated claim or act. | — |
| bleed | To cross a boundary slowly and without permission, in the stated metaphor. | — |
| block | To stop a thing from passing or from completing. | — |
| blue | The color that this set uses for external links. | — |
| bookkeeping | Record work that keeps counts and lists correct. | — |
| border | The line at which one area of rule meets another. | — |
| born | Created at a stated step or time. | — |
| borrow | To take and use a thing with the duty to give the same form back. | — |
| both | The two of a pair. | — |
| bound | A stated limit that a derivation or a search may not pass. | — |
| boundary | The line that separates two areas of rule. | — |
| brackets | The paired marks [ ] or ( ) around a text. | — |
| branch | A line of development in a repository. | — |
| breadth | The full width of a subject that a check covers. | — |
| breakaway | A process start that leaves the job that holds the parent. | — |
| breaks | Ends a rule, or stops working. | — |
| broken | Not working as the stated rule requires. | — |
| buffer | A held area of data that waits for use. | — |
| build | To make a program from its source. | — |
| built | Made from source. | — |
| busy | In use, or full of work. | — |
| but | Joins a contrast. | — |
| by | Through the action of, or next to. | — |
| byte | One unit of data of eight bits. | — |
| bytes | Units of data, eight bits each. | — |
| cache | A stored copy of a result, kept for a later read. | — |
| call | To invoke a function or a program. | — |
| callable | Able to be invoked as a function. | — |
| came | Past form of come. | — |
| camera | A device that records images, named in host examples. | — |
| can | To be able to. | — |
| cancel | To stop a planned action before it completes. | — |
| cannot | To be unable to. | — |
| canonical | In the one stated form that a record must take. | — |
| capability | A thing that a system can do. | — |
| captioned | Given a text label on a screen. | — |
| card | A small physical or on-screen credential item. | — |
| cargo | Cargo, the Rust build program. | — |
| carried | Held inside a value, or moved as part of a set. | — |
| carry | To hold a thing inside a value, or to move a result forward. | — |
| case | One stated condition or example in a set. | — |
| cease | To come to an end of a stated act. | — |
| cell | One box in a table, or one slot in a campaign plan. | — |
| central | Main, at the heart of a stated subject. | — |
| certificate | A signed record that binds a name to a key. | — |
| certified | Stated as passed by a stated check. | — |
| certify | To state formally that a check passed. | — |
| chain | A series of linked steps or names. | — |
| change | To make a thing different. | — |
| changelog | The version record file of the project. | — |
| channel | A named route by which one thing can reach another. | — |
| characters | Letters, digits, and marks in a text. | — |
| cheap | Lower in cost than another stated thing. | — |
| child | A process that one process starts directly. | — |
| choice | A thing that a person or a computation picks from a set. | — |
| choke | To block a flow at a narrow point, in the stated metaphor. | — |
| choose | To pick one from a set. | — |
| circular | Using a thing to prove itself. | — |
| citation | A reference to a stated record or text. | — |
| claim | A statement that its maker holds as true. | — |
| class | A set of things with a shared form. | — |
| classify | To place a thing in a stated class. | — |
| claude | Claude Code, a host product, in artefact names. | — |
| clause | One numbered part of a stated rule or hold. | — |
| clean | Free of failures, or free of reach in the stated check. | — |
| clear | Plain to read, or with nothing in the way. | — |
| click | A press of a pointer button on a screen item. | — |
| clipped | Cut short at a stated bound. | — |
| clips | Cuts short, or fastens. | — |
| clock | The machine's measure of passing time. | — |
| close | To shut a handle, a file, or a window. | — |
| closure | The act that ends a phase when its conditions are green. | — |
| code | The source text of a program. | — |
| codec | The part that encodes and decodes one format. | — |
| codex | Codex, a host product, in artefact names. | — |
| collect | To gather a stated thing as a recorded result. | — |
| collection | The act of gathering a stated thing as a recorded result. | — |
| collective | Shared by a whole set, taken as one. | — |
| colors | The hues that a page uses for its links. | — |
| com | The company suffix in a web address, in artefact text. | — |
| come | To arrive. | — |
| commit | A recorded change set in a repository. | — |
| company | The organization that owns a product. | — |
| compare | To examine two things for sameness or difference. | — |
| comparison | The act of examining two things for sameness or difference. | — |
| competent | Able to do the work correctly. | — |
| competitor | A rival product in a benchmark. | — |
| compiler | The program that turns source into a built program. | — |
| compiles | Turns source into a built program. | — |
| complete | With every part present. | — |
| completeness | The state of having every part accounted for. | — |
| completion | The end of a work, or the state of being complete. | — |
| computation | A calculation that a program does. | — |
| compute | To work out a value by stated steps. | — |
| concatenation | The joining of texts in order as one text. | — |
| concept | An idea that a text defines and uses. | — |
| concern | To be about a stated subject. | — |
| condition | A stated fact that must hold for a rule to apply. | — |
| conditional | A statement in code that chooses by a condition. | — |
| conditionals | Statements that choose by a condition. | — |
| configuration | The set of values that control a program, as data. | — |
| confirm | To state that a fact holds, after a check. | — |
| conformance | The state of meeting a stated standard's rules. | — |
| conformant | Meeting the stated rules of a standard or a list. | — |
| consent | A person's agreement, given for a stated act. | — |
| consequence | A result that follows from a stated fact. | — |
| console | The text window or device session that a program writes to. | — |
| consolidation | The act of bringing several homes or forms into one. | — |
| constant | A value in code that does not change. | — |
| construction | The way a thing is built. | — |
| consumer | A part that reads and uses a stated result. | — |
| consumes | Uses up a resource. | — |
| contain | To hold a thing inside a boundary. | — |
| containment | The act of holding a process inside a boundary. | — |
| content | The text or data that a file holds. | — |
| continuous | Without a break. | — |
| contract | A stated set of terms that two sides hold to. | — |
| contrast | A stated difference between two things, shown by placing them together. | — |
| contribute | To offer a change to a project. | — |
| contributions | Changes that people offer to a project. | — |
| contributor | A person who offers changes to a project. | — |
| control | A means to direct or to hold a thing within bounds. | — |
| convenience | Ease of use, taken as a reason for a choice. | — |
| convention | An agreed way of writing or placing a thing. | — |
| converge | To come to the same stated state from different starts. | — |
| converse | The reverse statement of a stated rule. | — |
| conversion | The act of changing a record from one form to another. | — |
| convert | To change a record from one form to another. | — |
| convict | To prove a stated party at fault by evidence. | — |
| cool | In artefact prose, a stated quality of a host feature. | — |
| copied | Made as an exact second form. | — |
| copy | An exact second form of a thing. | — |
| copyright | The legal right that an author holds in a work. | — |
| core | The central part of a system. | — |
| corpus | A set of texts or cases that a check reads as its body. | — |
| correct | True to the rule or the fact. | — |
| cost | What a run spends: time, work, or money. | — |
| could | Past or possible form of can. | — |
| count | To find the number of a set. | — |
| courtesy | A polite act that a rule does not require. | — |
| cover | To include a subject in a check, or to place over. | — |
| coverage | The breadth of a subject that a check or a record holds. | — |
| cpu | CPU, the central processor, in artefact text. | — |
| crafted | Made by hand for a stated purpose, case by case. | — |
| crash | A sudden end of a program on a fault it cannot hold. | — |
| crate | A Rust package of source that builds as one unit. | — |
| create | To make a new thing. | — |
| creation | The act of making a new process or file. | — |
| credential | A secret or token that proves an identity. | — |
| criteria | The stated tests that a decision must pass. | — |
| criterion | One stated test that a decision must pass. | — |
| cross | To go from one side of a boundary to the other. | — |
| ctrl | The Control key, in artefact names. | — |
| curate | To choose by hand what a set holds. | — |
| curation | The act of choosing by hand what a set holds. | — |
| current | Of the present time or version. | — |
| cursor | Cursor, a host product and editor. | — |
| cyrillic | The Cyrillic script, in artefact text. | — |
| data | Information that a program reads or writes. | — |
| dataset | A set of data that a run reads or offers as one body. | — |
| dead | Not able to be reached or used, of a path or a code form. | — |
| decide | To choose after consideration. | — |
| decision | A choice with its reason. | verdict (for this) |
| declare | To state a name or a fact formally. | — |
| decompose | To split a thing into its stated parts. | — |
| decomposition | The act of splitting a thing into its stated parts. | — |
| deep | Going far down in a stated structure. | — |
| default | The value a program uses when no value is given. | — |
| defect | A fault in an artefact against its rule. | — |
| defective | With a fault against its rule. | — |
| defence | A protection against an attack. | defense (spelling variant) |
| defend | To argue or to act in support of a stated thing. | — |
| deferred | Done at a later stated point, not at the start. | — |
| define | To state the meaning of a word or the form of a thing. | — |
| defines | States the meaning of. | — |
| definition | The statement of what a word or a shape means. | — |
| delete | To remove a file or a record. | — |
| delta | A stated difference between two counts or states. | — |
| demote | To move a thing to a lower stated place. | — |
| denial | An answer that refuses an action. | — |
| denied | Refused by a stated rule. | — |
| denotation | The thing that a name stands for. | — |
| denote | To stand for a stated thing, of a name. | — |
| deny | To refuse an action. | — |
| depend | To need a thing in order to work. | — |
| dependency | A thing that a program needs in order to work. | — |
| depth | How deep a structure goes. | — |
| derivation | The steps that produce a result from stated facts. | — |
| derived | Produced by stated steps from other facts. | — |
| descendant | A process that comes from an ancestor at any depth. | — |
| describe | To state in words what a thing is or does. | — |
| description | A short text that states what a thing is. | — |
| descriptor | A value that names an open resource to the system. | — |
| design | The argued shape of an artefact. | — |
| desktop | The machine's main screen area, or the machine on a desk. | — |
| destroy | To end a thing so that it cannot be used again. | — |
| destroys | Ends a thing so that it cannot be used again. | — |
| destruction | The act of ending a thing so that it cannot be used again. | — |
| detect | To find a stated change or condition by a check. | — |
| detection | The act of finding a stated condition. | — |
| developer | A person who writes programs. | — |
| development | The work of writing and changing programs. | — |
| deviation | A stated measure of how far values stand from a central value. | — |
| device | A physical part of a machine, or a whole machine. | — |
| diagnostics | The output that a program writes to explain its state. | — |
| dialog | A small window that asks a person for an answer. | — |
| dict | Short artefact form for a dictionary value in code examples. | — |
| dictionary | The file or page that lists approved words and meanings. | — |
| did | Past form of do. | — |
| die | To stop running and not return. | — |
| diff | A stated record of the changes between two forms of a text. | — |
| difference | The way two things are not the same. | — |
| different | Not the same. | — |
| differential | That compares two forms of the same thing to find a difference. | — |
| differs | Is not the same. | — |
| digit | One written sign of a number, 0 to 9. | — |
| dimension | One stated axis of a campaign or a measure, with its own state. | — |
| dims | Makes a screen less bright in a stated display act. | — |
| direct | In a straight line, with no step between. | — |
| direction | The way in which a proof or a movement goes. | — |
| directory | A folder on disk. | folder (same meaning; folder is the form this set uses in prose) |
| disable | To turn a function off. | — |
| disagree | To give a different answer from another stated party. | — |
| discipline | A stated way of working that a check holds. | — |
| disconnect | To end a connection. | — |
| discover | To find a thing that exists but was not known. | — |
| discoverable | Able to be found by a stated read or search. | — |
| discovery | The act of finding what exists. | — |
| dispatch | To send a call to the part that handles it. | — |
| display | The screen output of a machine. | — |
| dispose | To place a thing in its final stated home. | — |
| disposition | A stated ruling on what is done with a stated item. | — |
| distance | How far apart two things are. | — |
| distinct | Separate and not the same, of two or more things. | — |
| diverge | To come to different answers or states over time. | — |
| divergence | A point at which two forms give different answers. | — |
| do | To perform an action. | — |
| docs | The documentation set, or its folder. | — |
| document | A file of text that states facts or rules. | — |
| documentation | The set of texts that teach a product. | — |
| domain | An area of rule or of names. | — |
| door | The entry page or file that a reader starts from, in the stated metaphor. | — |
| double | Twice as much, or in two copies. | — |
| doubt | A stated uncertainty about a claim. | — |
| down | Toward a lower place or state. | — |
| dr | DR, the letters that start a decision record identifier, in artefact text. | — |
| drawn | Laid out in the stated form. | — |
| draws | Pulls a conclusion from facts, or makes a line. | — |
| dressed | Presented as a stated thing that it is not. | — |
| drift | To move slowly away from a stated state without a stated act. | — |
| drive | A disk device, or a storage area with a letter. | — |
| drop | To let a thing fall from a stated set, or to give it up. | — |
| dropdown | A page part that opens to show more text. | — |
| duplicate | An exact second copy of a thing. | — |
| duty | A stated thing that a party must do. | — |
| each | Every one, taken one by one. | — |
| earlier | Of a time or form before the stated one. | — |
| early | Before the expected time. | — |
| earn | To gain a place by meeting a stated test. | — |
| earns | Gains a place by meeting a stated test. | — |
| easy | Simple to do or to state. | — |
| echo | To write back the characters that a person types. | — |
| edge | The border line of an area or a case. | — |
| edit | To change a text. | — |
| edition | One stated version of a standard or a text. | — |
| editor | A program that a person uses to change text. | — |
| eight | The number 8. | — |
| either | One or the other of a pair. | — |
| else | Otherwise, or in another case. | — |
| embedder | A program that holds another program inside itself. | — |
| empty | With nothing inside. | — |
| enclosing | Around a thing on all sides. | — |
| encoded | Written in a stated code form. | — |
| encoding | The form in which text or data is written as bytes. | — |
| enforcement | The act of making a rule hold. | — |
| engine | The module that runs the central work of the guard. | — |
| english | The language of this documentation set. | — |
| enough | As much as a stated need requires. | — |
| enter | To go into a place or a state, or to type a value. | — |
| entry | One item in a list, a table, or a record. | — |
| enumerate | To list every item of a set in order. | — |
| enumeration | A list of every item of a set, in order. | — |
| environment | The set of values that a system gives a process at its start. | — |
| equal | The same in value or in form. | — |
| equality | The state of being the same in value or in form. | — |
| equals | Is the same in value as. | — |
| equivalence | The state of two forms holding the same value or effect. | — |
| erase | To remove a record so that no trace of it stands. | — |
| error | A fault state that a program reports. | refusal (for this) |
| esc | The Escape key, in artefact names. | — |
| esc | The Escape key name, counted here as a name. | — |
| escape | To leave a boundary by a route the rule did not state. | — |
| establish | To set a fact in record by a stated run or check. | — |
| established | Set up and proven over time. | — |
| estimate | To work out a value from stated measures, not by a direct count. | — |
| estimator | The stated computation that works out a value from stated measures. | — |
| event | A thing that happens at a stated time. | — |
| ever | At any time. | — |
| every | All of a set, without exception. | — |
| evidence | Records that show a fact. | — |
| exact | Precisely the stated value, with no difference. | — |
| example | One case that shows a rule. | — |
| exceeds | Is more than a stated bound. | — |
| except | Not including. | — |
| exception | A stated case that a rule does not bind. | — |
| exclude | To leave a stated thing out of a set. | — |
| exclusion | A stated case that a list leaves out, with its reason. | — |
| executable | A built program file that a machine can run. | — |
| execute | To run a stated program or step. | — |
| executes | Runs a program or a command. | — |
| executing | Running now. | — |
| execution | The act of running a program. | — |
| exercise | To put a function to real use in a test. | — |
| exhaustion | The state in which a bounded search has used all its stated bound. | — |
| exist | To be present as a stated thing. | — |
| existence | The state of being present. | — |
| exists | Is present. | — |
| exit | The end of a process, or the code it returns. | — |
| expect | To look for a stated thing as the likely result. | — |
| expected | Stated in advance as the result that must come. | — |
| explain | To state the reason for a stated thing in words. | — |
| explicit | Stated in full, with nothing left to be guessed. | — |
| export | Data that a program writes out for another program. | — |
| extend | To make a thing longer or larger. | — |
| extendable | Able to be made longer by a stated mechanism. | — |
| external | Outside the boundary in question. | — |
| extra | More than the stated set. | — |
| extract | To take a stated file or value out of a set. | — |
| extraction | The act of taking a stated thing out of a set. | — |
| extreme | At the far end of a stated range. | — |
| fabricate | To make a stated signal or record that is not true. | — |
| fabrication | A stated thing made to look true when it is not. | — |
| facade | An outer form that hides a second structure behind it. | — |
| face | The public form of a thing. | — |
| fact | A statement that is true and can be checked. | — |
| fail | To not meet a stated test. | — |
| failure | The state of not meeting a stated test. | standing loss (for a recorded campaign loss) |
| fake | Made to look true when it is not. | — |
| fallback | A second route that a program takes when the first route fails. | — |
| false | Not true. | — |
| far | At a great distance. | — |
| fast | Taking little time, of a stated path. | — |
| fault | A stated defect in a record or a form. | — |
| feature | A stated capability of a product. | platform mechanism (for this, in seam prose) |
| feed | To supply a stated thing to a computation or a record. | — |
| feeling | A sense that a thing is so, with no stated measure. | — |
| fell | Past form of fall: came down by a stated amount. | — |
| fence | A stated boundary in the record, in the stated metaphor. | — |
| fetch | To get a stated value or object from the system. | — |
| field | One named part of a record or a form. | — |
| fifth | One of five equal parts, or the item after the fourth. | — |
| file | A named unit of data on disk. | — |
| filename | The name of a file, with its stated form. | — |
| fill | To put a value into every stated place. | — |
| final | Last, after which nothing in the set changes. | — |
| find | To discover a thing that exists. | — |
| findable | Able to be found by a reader or a check. | — |
| finding | A stated result of a review or a check. | — |
| finds | Discovers a thing that exists. | — |
| fingerprint | A short value that identifies a key or a file. | — |
| finish | To complete a work. | — |
| finite | With a stated end. | — |
| first | Before all others. | — |
| fit | To be of the right size or shape for a stated place. | — |
| five | The number 5. | — |
| fix | A change that cures a defect. | — |
| flag | A named option on a command line, or a stated bit in an object. | — |
| flat | In one level, with no folders inside. | — |
| focus | The screen item that receives typed input. | — |
| fold | To place a whole tree into another stated home, part by part. | — |
| folder | A container on disk that holds files. | directory (for this, in prose) |
| follow | To come after, or to obey a stated rule. | — |
| footnote | A note at the foot of a page or a record. | — |
| for | Intended for, or because of. | — |
| force | To make a stated thing happen by a stated link or rule. | — |
| foreign | From outside the stated system or party. | — |
| forged | Made to look true when it is not, of a packet or a record. | — |
| fork | A point where a rule gives two stated routes. | — |
| form | The shape or the spelling of a thing. | — |
| formulation | One stated form in which a computation is written. | — |
| forward | In the direction in which a proof or a text goes. | — |
| found | Discovered to exist. | — |
| four | The number 4. | — |
| free | To release a stated object so that the system can use it again. | — |
| fresh | New to a stated work, with no earlier part in it. | — |
| from | Starting at. | — |
| front | The forward part, or the first screen a reader sees. | — |
| full | With every part present, or at the stated maximum. | — |
| fully | Completely. | — |
| function | A named unit of code that takes values and can return a value. | — |
| future | The time after now. | — |
| gain | To get a stated thing as a result. | — |
| gap | A stated place where a required item is absent. | — |
| gate | The role or check that allows or denies at a boundary. | — |
| gave | Past form of give. | — |
| general | True for a whole class, not one case. | — |
| generated | Produced by a program from stated inputs. | — |
| generator | A program that produces a stated file from stated inputs. | — |
| generic | True for any type in a stated class. | — |
| get | To obtain a value. | — |
| git | Git, the version control program. | — |
| github | GitHub, the service that hosts the repository. | — |
| give | To hand a value to a receiver. | — |
| given | Stated as a fact to start from. | — |
| glance | A quick look. | — |
| glob | A pattern that supplies every matching name at once, in an import or a re-export. | — |
| glossary | The page that lists the approved words and their meanings. | — |
| gnu | GNU, in artefact names such as the windows-gnu target. | — |
| go | To move or to proceed. | — |
| gone | No longer present. | — |
| google | Google, a company name in artefact text. | — |
| govern | To be the stated rule that holds over a subject. | — |
| graceful | Done in an orderly stated step, not by a sudden end. | — |
| grant | To give a permission or a hold formally. | — |
| granular | Stated at the grain of one file. | — |
| graph | The page or data that shows how parts connect. | — |
| greater | Larger in value. | — |
| green | Passed, in the stated test sense. | — |
| grew | Past form of grow. | — |
| grow | To become larger, or to add stated code over time. | — |
| guarantee | A promise that a product states and keeps. | — |
| guess | To state a thing without a stated check of it. | — |
| guides | Texts that teach a reader step by step. | — |
| had | Past form of have. | — |
| half | One of two equal parts of a thing. | — |
| halt | To bring a stated computation to an end. | — |
| hand | A person's hand, or the act of giving control. | — |
| handle | A value that names an open object to the system. | — |
| hang | To wait without progress and without an end. | — |
| happens | Takes place. | — |
| hard | Difficult to do or to find. | — |
| hardened | Made harder to attack by a stated work. | — |
| hardening | The work that makes a process harder to attack. | — |
| hardens | Makes a process harder to attack. | — |
| harness | The test program that runs cases and records results. | — |
| has | Holds or possesses. | — |
| hash | A short fixed-length value that a function computes from data. | checksum (for a SHA-256 file value) |
| have | To hold or possess. | — |
| head | The top part, or the latest commit of a branch. | — |
| headline | Stated as the main number or claim. | — |
| held | Held inside a value, or kept by a named check. | — |
| hello | The greeting ceremony in artefact names for the sign-in check. | — |
| help | Text that states how to use a program. | — |
| helpers | Small units of code that support a main unit. | wrapper (for a seam wrapper) |
| here | At this place. | — |
| hidden | Kept from view by a stated form. | — |
| hides | Keeps a thing from view. | — |
| historical | Of a past version or a past state. | — |
| history | The recorded past of a thing, kept in commit history. | — |
| hole | A stated place where a required part is absent. | — |
| home | The folder or page that a thing belongs to. | — |
| honest | Stating only what the evidence holds. | — |
| honesty | The quality of stating only what the evidence holds. | — |
| hook | A defined point where a host calls the guard. | — |
| hop | One step in a following chain, or one link between pages. | — |
| hostile | Trying to break a rule or to harm. | — |
| hours | Units of time, 60 minutes each. | — |
| how | In what way. | — |
| hundred | The number 100, used for a count in the hundreds. | — |
| identical | The same in every stated byte or part. | — |
| identification | The act or the record that names who a person is. | — |
| identifier | A value that names one thing in a set. | — |
| identity | The stated nature of a product that makes it itself. | — |
| if | In the case that. | — |
| image | The built form of a program as the linker writes it. | — |
| immediate | At once, with no wait. | — |
| impl | Short artefact form for an implementation in code names. | — |
| implausible | Not able to be believed under the stated rules. | — |
| implement | To build a stated step or form so that it runs. | — |
| implicit | Present without being stated. | — |
| implied | Present in effect, but not stated. | — |
| import | A statement that brings a name into a module. | — |
| impossible | Not able to happen or to be done. | — |
| in | Inside, or during. | — |
| incomplete | With a stated part absent. | — |
| inconsistency | A state in which two computations give different answers on one fact. | — |
| incorporation | The act of folding a stated file or tree into another repository. | — |
| incorrect | Not correct. | — |
| independent | Not depending on another stated thing. | — |
| index | A list that points to the parts of a set. | manifest (for the layout file) |
| indirection | A route that reaches a thing through another thing. | — |
| indistinguishable | Not able to be told apart from a stated thing. | — |
| inert | Having no effect on a stated result. | — |
| inflate | To make a count larger than the stated facts give. | — |
| influence | To change a stated result by a stated field or act. | — |
| information | Facts that a stated act gives to a reader. | — |
| inheritance | The rule by which a type takes the members of another type. | — |
| initialiser | The expression that gives a value at its definition. | initializer (spelling variant) |
| inline | Written in place, inside another item. | — |
| inside | In the interior of. | — |
| inspect | To look at a stated thing closely by a check. | — |
| install | To put a program on a machine in its working place. | — |
| instance | One stated case of a thing. | — |
| instant | One stated moment in a run. | — |
| instead | In place of another thing. | — |
| instruction | One stated action that a reader or a program must do. | — |
| instrument | A tool of the work: a program that checks or measures. | — |
| integration | The whole connection between the guard and one host. | — |
| integrator | A person or program that connects two systems. | — |
| intent | The stated purpose that a check or a text carries. | — |
| intention | A stated plan to do a stated act. | — |
| interactive | With a person able to answer during the run. | — |
| interface | The stated surface at which two parties meet. | — |
| intermediate | Between the start and the final form. | — |
| internal | Inside the boundary in question. | — |
| into | To the interior of. | — |
| intree | In the tree: of the tests that live in the tree itself. | — |
| inventory | A stated list of every item in a set. | — |
| io | IO, input and output, in artefact text. | — |
| is | Third person present form of be. | — |
| issue | A reported problem, or a point in question. | — |
| it | The thing just named. | — |
| item | One thing in a list or a set. | — |
| itself | The same thing, with no other actor. | — |
| iv | The Roman numeral 4, in artefact text such as a gate item label. | — |
| joined | Connected into a set or a network. | — |
| joins | Connects into a set. | — |
| json | The data format of the manifest and the payloads, in artefact use. | — |
| judge | To give a stated answer on a claim by a stated check. | — |
| judged | Examined by a named check against a stated rule. | — |
| judgement | A stated answer given on a claim after a check. | — |
| keep | To hold a thing in its state. | — |
| kept | Held in its state. | — |
| key | A named field in the manifest, or a cryptographic secret, or a keyboard button. | — |
| keyboard | The physical keys that a person types on. | — |
| kind | A stated class of a thing. | — |
| knew | Past form of know. | — |
| know | To have a stated fact in record or in mind. | — |
| known | Identified and recorded. | — |
| knows | Has a fact in record. | — |
| lab | The repository or work area in artefact names. | — |
| label | A short name placed on a thing. | — |
| landed | Past form of land: placed a change into the tree. | — |
| landing | The act that places a change into the tree. | — |
| language | A system of words, or a programming language. | — |
| laptop | A portable computer. | — |
| large | Great in size or in number. | — |
| largest | Greatest in value in a set. | — |
| last | After all others. | — |
| later | At a time after. | — |
| latest | Most recent. | — |
| latin | The Latin script, in artefact text. | — |
| launch | To start a program or a run. | — |
| layered | Placed in levels, one over another. | — |
| layout | The arrangement of files and folders in a tree. | — |
| lazily | At first use only, not at the start. | — |
| leading | At the front. | — |
| leaf | The last item in a branch of a structure. | — |
| leak | A loss of a stated resource with no stated bound. | — |
| learn | To gain knowledge from a text or a run. | — |
| leave | To go away from, or to let a thing stay. | — |
| ledger | A stated record that lists items and their rulings. | — |
| left | On the side that is west when a person faces north; also, remaining. | — |
| less | Smaller in amount. | — |
| let | To permit. | — |
| letters | Characters of an alphabet. | — |
| library | A set of code that other code calls. | — |
| licence | The legal permission to use a work. | license (spelling variant, for this) |
| license | The legal permission to use a work, in artefact spellings. | licence (for this, in prose) |
| life | The time during which a process or a record exists. | — |
| lifetime | The full time during which a thing exists. | — |
| like | Similar to. | — |
| line | One row of text in a file. | — |
| link | A page element that takes a reader to another place. | — |
| lint | The check program that holds the documentation rules. | — |
| linux | Linux, an operating system. | — |
| list | A set of items written in order. | ring (for the census ring) |
| literal | A stated path or value written in full, not by a name. | — |
| live | Running now, or in real use. | — |
| load | The work that a machine carries, or the act of reading data in. | — |
| local | On this machine, not remote. | — |
| location | The place where a thing is. | — |
| lock | A control that gives one holder at a time the right to proceed. | — |
| lockout | A state in which sign-in is refused after failed attempts. | — |
| log | The record that a program writes as it runs. | — |
| logic | The rules by which a computation decides. | — |
| logon | The act of signing in to a machine. | — |
| long | Of great length or time. | — |
| look | To examine with the eyes or by a check. | — |
| loose | Not exact in stated form. | — |
| loosely | In a form that is not exact. | — |
| lose | To stop having a stated thing. | — |
| loss | A campaign cell that the checker counts against the guard. | — |
| lost | Not held any more, after a stated act. | — |
| loud | Stated in a way that no reader can miss. | — |
| loudly | In a way that no reader can miss. | — |
| low | Below the stated value, of a count. | — |
| mac | Mac, a computer that runs macOS, in artefact text. | — |
| machine | A computer. | — |
| machinery | Standing parts that a path carries for its work. | — |
| macos | macOS, an operating system. | — |
| macro | A code form that expands into other code at build time. | — |
| made | Built or produced. | — |
| main | Chief, the one that carries the work. | — |
| make | To build or to cause. | — |
| malformed | Not in the required form. | — |
| management | The work of controlling a system or a service. | — |
| manager | A program or a person that controls a set. | — |
| mandatory | Required by a rule. | — |
| manual | Done by a person, or the book that teaches a system. | — |
| manufacture | To make a stated thing by a stated process. | — |
| many | A large number of. | — |
| map | A table that states which item goes with which item. | — |
| marker | A stated sign that shows a place or a state. | — |
| match | To be the same as a stated pattern or value. | — |
| matrix | A table of cells that shows two sets against each other. | — |
| matter | A subject under consideration, or the substance of a text. | — |
| maximized | Made as large as the screen allows. | — |
| may | To be permitted to, or to be possible. | — |
| mean | To have a stated thing as its meaning or its result. | — |
| meaning | What a word or a statement states. | — |
| means | States as its meaning, or makes possible. | — |
| measure | To find a size, a time, or a count by a stated method. | — |
| measurement | The act or the result of measuring. | — |
| mechanical | Done by a program from stated rules, with no judgement. | — |
| mechanism | A stated means by which a system does a thing. | — |
| median | The middle value of a stated set of numbers. | — |
| mediated | Passed through a stated middle part. | — |
| meet | To come to a stated thing, in fact or in a check. | — |
| member | One item or party in a stated set. | — |
| membership | The state of being in a stated set, or the set itself. | — |
| memory | The fast storage that a running program uses. | — |
| mended | Repaired. | — |
| merge | To combine two sets or two branches into one. | — |
| message | A unit of text or data that one part sends to another. | — |
| method | A stated way of doing a thing. | — |
| mib | MiB, the mebibyte unit, in artefact text. | — |
| microsoft | Microsoft, the company that makes Windows. | — |
| mingw | MinGW-w64, a build toolchain name in artefact text. | — |
| minimal | The smallest that a stated purpose allows. | — |
| minimum | Smallest that a rule allows. | — |
| minute | A unit of time, 60 seconds. | — |
| missed | Not reached or not recorded. | — |
| missing | Absent where a rule needs a thing present. | — |
| mixed | Made of two or more stated kinds together. | — |
| model | A described form that other things follow. | — |
| module | One file or one stated unit of code. | — |
| moment | A short time, or one stated point in a run. | — |
| more | Larger in amount. | — |
| most | Greatest in amount. | — |
| move | To change the place of a thing. | — |
| movement | The act of moving a thing from one place to another. | — |
| msvc | MSVC, the Microsoft build toolchain name in artefact text. | — |
| multiply | To make a stated thing exist in many places. | — |
| multiset | A set in which the count of each item is part of the fact. | — |
| must | To be required to. | — |
| mutated | Changed in a stated way as a test case. | — |
| name | A word that identifies a thing. | — |
| narrow | Small in stated breadth. | — |
| native | Belonging to the system itself, not added. | — |
| nearest | Closest in distance or in a stated order. | — |
| need | To require a thing. | — |
| negative | Of a control case that must give the stated failing answer. | — |
| neither | Not one and not the other of a pair. | — |
| nest | To place one axis or thing inside another of the same kind. | — |
| nested | Placed inside another thing of the same kind. | — |
| network | A set of connected machines. | — |
| neutral | Neither for nor against a stated claim. | — |
| never | At no time. | — |
| new | Made or found for the first time. | — |
| next | Immediately after the present one. | — |
| no | Not any. | — |
| nobody | No person. | — |
| noise | Variation in a measure that no stated fact explains. | — |
| noisy | With much variation that no stated fact explains. | — |
| non | Prefix form in artefact words: not. | — |
| none | Not any of a set. | — |
| nor | And not. | — |
| normal | In the usual state. | — |
| not | Makes a statement negative. | — |
| note | A short recorded statement. | — |
| nothing | No thing. | — |
| notice | A written statement that a program gives a user. | — |
| noun | A word that names a person, a place, or a thing. | — |
| now | At the present time. | — |
| nowhere | At no place. | — |
| null | With no value, in data. | — |
| number | A value that counts or measures. | — |
| numeric | Stated as a number. | — |
| object | A thing that a system creates and names, such as a job object. | — |
| occurrence | One time that a thing appears. | — |
| of | Belonging to, or from a set. | — |
| off | In the stopped state, or away from. | — |
| offers | Gives a choice or a capability. | — |
| old | Of an earlier stated time or form. | — |
| on | Atop, or in the operating state. | — |
| once | One time only. | — |
| one | The number 1. | — |
| onward | Toward a later point. | — |
| open | Not closed, or available to read. | — |
| operating | In the phrase operating system: the base software of a machine. | — |
| operation | One stated act that a system performs. | — |
| option | One choice in a set that a command or a person can take. | — |
| optional | Present only when a stated caller asks for it. | — |
| or | Joins alternatives. | — |
| order | The sequence in which items stand. | — |
| org | Short artefact form for the organization area of the record. | — |
| organization | The arrangement of parts, or the body that owns a work. | — |
| organize | To place items under a stated axis or order. | — |
| origin | The place or value from which a thing comes. | — |
| original | First, before any change. | — |
| other | Different from the one named. | — |
| otherwise | In another case. | — |
| out | Outside, or into the open. | — |
| outcome | The result of a stated act or check. | — |
| outlive | To exist longer than another thing. | — |
| output | What a program writes out. | — |
| outside | Beyond a boundary. | — |
| over | Above, or more than. | — |
| own | Belonging to the subject itself. | — |
| package | A set of files that ships as one unit. | — |
| packet | A stated unit of data that one part sends to another. | — |
| page | One document in the wiki set, or one screen of text. | — |
| paid | Past form of pay. | — |
| pair | Two stated things that a rule joins as one. | — |
| pairs | Two items that a rule joins. | — |
| panic | A state in which a program stops at once on a fault it cannot hold. | — |
| papers | Working documents of a stated work. | — |
| parameter | A value that controls a computation, in code. | — |
| parse | To read a text into its stated parts. | — |
| part | A piece of a whole. | — |
| partial | Of one part only, not the whole. | — |
| party | A person or body in an agreement or an attack. | — |
| pass | To meet a stated test, or to hand a value onward. | — |
| password | A secret word or phrase that proves an identity. | — |
| past | The time before now. | — |
| path | The name of a file's place, with its folders. | — |
| pay | To give a stated cost for a thing. | — |
| per | For each one. | — |
| perfect | With no defect against the stated test. | — |
| permanent | Lasting, not removed by a later step in the set. | — |
| person | A human being. | — |
| personal | Belonging to one person. | — |
| peter | Peter Shiller, the author named in the copyright line. | — |
| phase | One stated stage of a work. | — |
| phrase | A group of words that acts as one term. | — |
| picked | Chosen. | — |
| pieces | Small parts of a whole. | — |
| pipe | A channel that carries one program's output to another's input. | — |
| place | A stated location. | — |
| placeholder | A stated token that a later step replaces with a value. | named place (for a folder) |
| placement | The act of putting a thing in its place. | — |
| plain | Simple and clear, with nothing hidden. | — |
| plan | A stated set of cells, steps, or acts that a work follows. | — |
| planted | Placed on purpose as a test defect. | — |
| plausible | Able to be believed under the stated rules. | — |
| plus | With a stated thing added. | — |
| point | A stated item in an argument, or a place. | — |
| pointer | A value that names the place of a stated thing. | — |
| policy | The installed policy text that the guard prints and enforces. | policy file (for the file as data) |
| populated | Holding the stated files or items. | — |
| popup | A small window that appears over a screen. | — |
| portable | Able to be carried to another stated system. | — |
| position | The place that a thing holds. | — |
| possible | Able to happen under the stated rules. | — |
| powerful | With great capability. | — |
| powershell | PowerShell, a shell program. | — |
| pre | Prefix form in artefact words: before. | — |
| precision | The stated fineness of a measure or a claim. | — |
| predate | To stand at a time before a stated thing. | — |
| predicate | A stated condition that a claim must meet, as a whole. | — |
| predicted | Stated in advance as the expected result. | — |
| predicts | States in advance what a reader or a rule expects. | — |
| preference | A choice made for taste, not by a stated rule. | — |
| premise | A stated fact that an argument takes as its start. | — |
| prerequisites | Things that must be present before a step starts. | — |
| presence | The state of being present. | — |
| present | Here now. | — |
| preservation | The act of keeping a stated thing in its form for later reading. | — |
| preserve | To keep a stated thing in its form. | — |
| preserved | Kept in its form for later reading. | — |
| press | To push a key or a button. | — |
| pretooluse | The PreToolUse hook event name in artefact text. | — |
| prevent | To keep a stated act from happening. | — |
| price | The cost of a thing, or a stated value in a record. | — |
| principal | The main person or process in a stated rule. | — |
| principle | A first rule from which other rules come. | — |
| print | To write a text to a screen or a stream. | — |
| private | Not open to others. | — |
| problem | A stated difficulty that a work must answer. | — |
| procedure | A stated series of steps for one task. | — |
| proceed | To go forward in a stated order of steps. | — |
| process | One running instance of a program. | — |
| process | One running instance of a program. | — |
| processor | The part of a machine that runs computations. | — |
| producer | A part that writes stated records or values. | — |
| product | The artefact that a project ships. | — |
| production | In real use, or of the shipped source set. | — |
| professor | A teacher at the stated level, in the anchor's test. | — |
| program | A set of instructions that a machine runs. | — |
| project | A stated body of work with one goal. | — |
| promise | A statement that a maker holds as a duty. | — |
| prompt | A window or a line that asks a person for a value. | — |
| pronoun | A word that stands for a named thing. | — |
| proof | Evidence that shows a claim is true. | — |
| proper | In the phrase proper noun: naming one stated person or place. | — |
| property | A stated quality of a thing. | — |
| proportion | A part stated against the whole. | — |
| propose | To state a thing for a stated decision. | — |
| prose | Written text that is not code, not a table of data, and not a heading. | — |
| protected | Kept safe by a stated means. | — |
| protection | The means that keeps a thing safe. | — |
| protects | Keeps a thing safe. | — |
| protocol | The stated rules by which two parties exchange data. | — |
| provably | In a way that a stated proof holds. | — |
| prove | To show a claim is true by evidence. | — |
| proven | Shown true by evidence at the stated tier. | — |
| provenance | The stated origin of a file or a fact. | — |
| provides | Gives a capability or a value. | — |
| pseudo | In artefact names: not a real instance, a stated stand-in. | — |
| ptg | PTG, short form of the product name in PTG-STE-1. | — |
| public | Open to all readers. | — |
| pull | To fetch changes from a repository. | — |
| pure | With a census count of zero, or with nothing mixed in. | — |
| purity | The state of having no stated reach or mixture. | — |
| purpose | The reason for which a thing exists. | — |
| put | To place a thing. | — |
| python | Python, a programming language named in suite prose. | — |
| qualification | A stated limit that keeps a claim at its true breadth. | — |
| quartile | One of the four equal parts into which a set of values is split, used as a range. | — |
| query | A stated call that asks a system for a fact. | — |
| question | A sentence that asks for an answer. | — |
| queue | A line of items that wait for service in order. | — |
| quota | A stated count that a party keeps by hand as a target. | — |
| race | A fault in which two acts reach a shared state in either order. | — |
| raise | To make a stated error state occur. | — |
| ran | Past form of run. | — |
| random | Chosen with no stated pattern. | — |
| range | The stated breadth between the smallest and the largest value. | — |
| rate | A stated amount per unit of time or of work. | — |
| raw | In its first form, not yet worked on. | — |
| re | Prefix form in artefact words: again, or back. | — |
| read | To take in a text or a value. | — |
| readability | The quality of being easy to read correctly. | — |
| reader | A person or a part that reads a stated text or home. | — |
| readme | A README file: the sign of a folder, or the door of a repo. | — |
| real | Existing in fact, not in name only. | — |
| reason | The stated cause or ground for a thing. | — |
| rebind | To bind a name to a value again. | — |
| rebuild | The act of building a stated product again. | — |
| receiver | The part that receives a value or a call. | — |
| receives | Takes in a value or a call. | — |
| recent | Of a time just before now. | — |
| reclaim | To take a stated resource back for later use. | — |
| recognition | The act of identifying a person or a thing. | — |
| recompute | To calculate a value again from its facts. | — |
| record | A written statement that a run or a work keeps. | — |
| recovery | A stated surface or act that brings a thing back after a fault. | — |
| red | The color that this set uses for internal links. | — |
| reduce | To make a stated claim or value smaller in breadth. | — |
| reducible | Able to be made smaller in stated breadth. | — |
| reduction | The act of making a stated claim smaller in breadth. | — |
| reference | A pointer to another text or value. | — |
| refresh | To state a stated record again from a fresh run. | — |
| refuse | To decline to do a stated act. | — |
| regenerate | To produce a generated file again from its inputs. | — |
| registered | Recorded in a stated list in advance. | — |
| regression | A stated test set that holds earlier behaviour against a change. | — |
| relation | A stated connection between two things. | — |
| release | A stated version that a project ships. | — |
| remain | To stay in a stated place or state. | — |
| remote | On another machine, reached over a network. | — |
| removal | The act of taking a stated thing away. | — |
| remove | To take a stated thing away. | — |
| removes | Takes a thing away. | — |
| rename | The act of giving a stated thing a new name. | — |
| render | To show a stated thing in a stated state as a result. | — |
| renew | To extend a credential or a right for a new period. | — |
| renewal | The act of extending a credential for a new period. | — |
| reopens | Opens again. | — |
| reorganized | Placed again under a new arrangement. | — |
| repair | To fix a defect. | — |
| repeat | To do a thing again. | — |
| replace | To put a thing in the place of another. | — |
| replacement | A thing that takes the place of another. | — |
| repo | Short form for a repository. | — |
| report | A written statement of results or facts. | — |
| repository | The stored history and files of a project. | — |
| require | To need a stated thing by rule. | — |
| required | Needed by a rule. | — |
| requirement | A stated thing that a rule needs. | — |
| resident | Staying in place across many stated runs, of a process. | — |
| resolution | The act of finding the target of a name. | chase (for the census computation) |
| resolve | To come to a stated final state or answer. | — |
| resource | A thing that a run uses: time, memory, a handle, or a file. | — |
| rest | The remaining part of a set. | — |
| restore | To bring a thing back to a stated state. | — |
| restructure | To change the arrangement of parts. | — |
| result | What an act or a computation produces. | — |
| resumes | Starts again after a pause. | — |
| retired | Taken out of use at a stated step. | — |
| retirement | The act of taking a stated thing out of use. | — |
| retried | Past form of retry: done again after a failure. | — |
| return | To give a value back to a caller. | — |
| reuse | To use a thing again. | — |
| reuses | Uses a thing again. | — |
| reversal | An act that undoes a stated decision. | — |
| review | An examination of an artefact against its rules. | keeper (for a blocking check) |
| revives | Makes a held item live again. | — |
| rewritten | Written again in a new form. | — |
| right | Correct, or the side that is east when a person faces north. | — |
| rigour | Strict care in a stated method. | — |
| risk | The stated chance of a stated wrong result. | — |
| root | The top folder of a tree, or the base of a structure. | — |
| round | One complete cycle of a stated operation. | — |
| route | A stated path by which a thing reaches a target. | — |
| row | One horizontal line in a table or a file. | — |
| rule | A stated requirement that a check can hold. | — |
| runners | Programs that run tests or campaigns. | — |
| runtime | The time at which a program runs, in the phrase runtime product data. | — |
| rust | Rust, the programming language of the guard. | — |
| safe | Not able to cause the stated harm. | — |
| same | Identical in the stated respect. | — |
| sanity | A stated check that a work still stands on its stated facts. | — |
| satisfied | Met in full, of a stated condition. | — |
| saving | A stated cost that a change does not pay. | — |
| say | To state in words. | — |
| scale | The size of a test population, or a measure's range. | — |
| scan | An examination of each item in a set. | — |
| scheduled | Set to run at a stated time by a scheduler. | — |
| scheduler | The system part that starts programs at stated times. | — |
| schema | A stated form that data must take. | — |
| scheme | A stated way of naming or of placing things. | — |
| school | An institution of teaching, in examples. | — |
| scope | The stated set that a rule or a check covers. | — |
| screen | The display surface of a machine. | — |
| script | A file of commands that a program runs in order. | — |
| scrolling | The act of moving text on a screen. | — |
| seam | The boundary at which a body of code meets the platform. | — |
| search | An act that looks for a stated item in a set. | — |
| seat | A stated place in a capacity count, in examples. | — |
| second | After the first, or a unit of time. | — |
| secret | A value that only stated parties may know. | — |
| section | One stated part of a document under one heading. | — |
| secure | Protected against stated attacks. | — |
| security | The protection of a system against stated attacks. | — |
| see | To notice or to read. | — |
| seen | Noticed or read. | — |
| select | To choose one from a set. | — |
| selection | The act of choosing, or the part chosen. | — |
| self | Prefix form in artefact words: the thing itself. | — |
| sends | Causes a value to go to a receiver. | — |
| sensitivity | The quality of a check that lets it detect a stated change. | — |
| sent | Given to a receiver. | — |
| sentence | A group of words that states one complete thought. | — |
| sentinel | A stated rule or record that marks a missing stated thing. | — |
| separate | Not joined to another thing. | — |
| sequence | A stated order of steps or events. | — |
| series | A set of items in a stated order. | — |
| serve | To give a stated service to a caller. | — |
| server | A process that answers stated requests across many runs. | — |
| service | A program that runs in the background and answers callers. | — |
| set | A collection of items, or the act of placing a value. | — |
| settle | To reach a final state that no later step changes. | — |
| several | More than two, but not many. | — |
| shape | The arrangement of parts that a reader can see. | — |
| share | To give a part to others, or to hold in common. | — |
| shed | To lose a stated part, such as size or code. | — |
| shedding | The measured loss of size or code from a program. | — |
| shell | The program that reads commands and starts programs. | — |
| shiller | Shiller, part of the author name in the copyright line. | — |
| ship | To place a product into its final tree or release. | — |
| shipment | The act of shipping a stated product, or the set shipped. | — |
| short | Of small length or time. | — |
| shot | One single run, in the phrase one-shot. | — |
| should | To be the correct act under a stated rule. | — |
| show | To make a thing visible. | — |
| shown | Made visible. | — |
| sibling | An item at the same level next to another item. | — |
| side | One face of a boundary, or one party of a pair. | — |
| sidebar | The navigation part at the side of a wiki page. | — |
| sign | A stated mark, or the folder README as a folder's sign. | — |
| signal | A stated message that a system sends to a process. | — |
| silence | A review result with no open objection, in artefact prose. | — |
| silent | With no output and no open objection. | — |
| silicon | The material of chips, in the platform axis sense. | — |
| simplified | Made simpler under stated rules. | — |
| since | From a stated past time. | — |
| single | One only, taken alone. | — |
| singly | One at a time, taken alone. | — |
| sit | To be placed. | — |
| six | The number 6. | — |
| size | How large a thing is. | — |
| sketch | To draw a stated shape in first form. | — |
| skipped | Not run, with the reason stated. | unevaluated (for a dimension state) |
| slots | Stated places in a campaign plan that runs fill. | — |
| small | Of little size. | — |
| smart | With stated automatic functions, in artefact names. | — |
| so | To that degree, or therefore. | — |
| soak | A long stated run under load, as a campaign surface. | — |
| some | An stated but not counted amount. | — |
| someone | An unnamed person. | — |
| soon | At a near time. | — |
| sort | To place items in a stated order. | — |
| sound | In a state that a stated check accepts. | — |
| source | The origin of a value, or the code before a build. | — |
| span | In artefact text: a code span, the text between two backtick marks. | — |
| spawn | To start a new process from a running one. | — |
| speaks | Uses a stated protocol. | — |
| specific | Stated for one named case, not for a class. | — |
| specification | A text that states what a thing must do. | — |
| speed | How fast a thing runs. | — |
| spelled | Written with stated letters. | — |
| spelling | The letters that a word is written with. | — |
| spent | Past form of spend: used up. | — |
| split | To divide a set into stated parts. | — |
| spread | The stated breadth of a set of values around a central value. | — |
| square | With four equal sides, in the phrase square brackets. | — |
| src | The src folder name in the tree. | — |
| ssh | SSH, a remote sign-in protocol name in artefact text. | — |
| stable | Not changing under stated conditions. | — |
| stage | One step in a stated series. | — |
| stale | Older than the fact it states. | — |
| stand | To be in a stated state or place. | — |
| standard | In the stated common form, such as standard input. | — |
| start | To begin a run or an act. | — |
| state | The condition of a thing at a stated time. | — |
| statement | A sentence that states a fact or a rule. | — |
| static | Done from a stated text alone, with no run, of a check. | — |
| station | The interactive display seat of a session, in artefact prose. | — |
| stay | To remain in a place or a state. | — |
| ste | STE, in the name PTG-STE-1. | — |
| step | One stated act in a series. | — |
| stig | STIG, a security configuration standard name in artefact text. | — |
| still | Continuing in the same state. | — |
| stood | Past form of stand. | — |
| store | To keep a value for later use. | — |
| stream | A flow of data in one direction. | — |
| strength | The power of a protection or a rule. | — |
| string | A sequence of characters. | — |
| structural | Of the stated structure of a thing. | — |
| structure | The stated arrangement of the parts of a thing. | — |
| studio | Visual Studio, a build product name in artefact text. | — |
| style | The stated manner of writing or presenting. | — |
| subcommand | A named command inside a program's command set. | — |
| subfolder | A folder inside another folder. | — |
| subject | The thing that a text is about. | — |
| subset | A set that is part of a larger set. | — |
| substitute | A thing that takes the place of another for a stated use. | — |
| substitution | The act of putting one stated thing in the place of another. | — |
| subsystem | A stated part of a system with its own rules. | — |
| success | The state of meeting a stated goal. | — |
| successful | Having met the stated test. | — |
| such | Of the stated kind. | — |
| summarizes | States the main facts of a text in short form. | — |
| summary | A short statement of the main facts. | — |
| summoning | The act of calling a stated window or program forward. | — |
| supply | To give a stated thing to a stated receiver. | — |
| support | Help that a product gives its users, or a stated capability. | — |
| suppress | To keep a stated refusal or fact from standing. | — |
| sure | Certain. | — |
| surface | The part of a thing that others can see or call. | — |
| surroundings | The things around a stated place. | — |
| survive | To stay in a stated state through a stated test. | — |
| survives | Remains after a stated event. | — |
| suspend | To pause a process or a work for a stated time. | — |
| symbol | A stated name that a computation adds or reads. | — |
| synchronous | Done at once, in the same stated act, with no queue. | — |
| synonym | A word with the same stated meaning as another word. | — |
| sysinternals | Sysinternals, a program set name in examples. | — |
| system | A set of parts that work as one whole. | — |
| tab | A control that selects one page of a set, or the Tab key. | — |
| table | A set of rows and columns that states facts. | — |
| take | To get hold of a thing. | — |
| taken | Got hold of. | — |
| talk | To exchange statements, in the stated metaphor. | — |
| target | The thing that an act aims at. | — |
| teaches | Gives knowledge of a subject. | — |
| teaching | The work of giving knowledge. | — |
| teardown | The act that ends and removes a test setup. | — |
| technical | Of the stated engineering subject. | — |
| tell | To work out a stated fact from stated signs. | — |
| tells | States a fact to a reader. | — |
| temporary | For a short time only. | — |
| ten | The number 10. | — |
| term | A word or phrase with one stated meaning. | — |
| terminal | The window or device that shows a session's text. | — |
| test | A check that runs and records a result. | — |
| text | Written words. | — |
| than | Used in comparisons. | — |
| that | The stated one. | — |
| the | The stated one, already named. | — |
| their | Belonging to them. | — |
| them | The stated ones, as object. | — |
| then | At that time, or next in order. | — |
| theory | A stated claim that no run has proven yet. | — |
| there | At that place. | — |
| therefore | For that reason. | — |
| these | The stated ones near at hand. | — |
| they | The stated ones, as subject. | — |
| thin | With little between the parts, in the stated metaphor. | — |
| things | Objects or matters. | — |
| third | After the second. | — |
| this | The stated one near at hand. | — |
| those | The stated ones at a distance. | — |
| thousand | The number 1,000, used for a count in the thousands. | — |
| thread | One line of execution inside a process. | — |
| three | The number 3. | — |
| through | From one side to the other, by way of. | — |
| tie | A stated connection that two records must agree on. | — |
| tiers | Plural of tier. | — |
| tile | One window unit in a stated display arrangement. | — |
| time | The measure of duration, or one occasion. | — |
| timing | The stated times at which acts happen. | — |
| title | The name at the head of a page or a window. | — |
| to | Toward a place, a state, or a receiver. | — |
| today | On this day. | — |
| together | In one place or at one time, as one set. | — |
| too | In addition, or more than enough. | — |
| took | Past form of take. | — |
| toolchain | The set of programs that builds a product. | — |
| top | The highest part. | — |
| total | The sum of a set of values. | — |
| touch | Contact by a finger on a screen, or a small change. | — |
| toward | In the direction of. | — |
| tpm | TPM, the Trusted Platform Module, in artefact text. | — |
| trace | A recorded series of steps or events. | — |
| track | To keep a stated value in record as it changes. | — |
| trait | A stated set of functions that a type provides, in Rust. | — |
| travel | To go with a stated thing to another place. | — |
| treat | To act toward a thing in a stated way. | — |
| tree | The structure of processes, folders, or data with one root. | — |
| trial | One stated run of a stated test. | — |
| trick | A stated act that makes a result look other than it is. | — |
| tried | Attempted. | — |
| triplication | The state of one stated thing standing in three homes. | — |
| true | In agreement with the fact. | — |
| truncation | The act of cutting a text at a stated length. | — |
| trust | The stated reliance on a party or a record. | — |
| truth | The stated fact of a matter, in full. | — |
| try | To attempt an act. | — |
| turn | To change state, or to take one's stated place in order. | — |
| twelfth | The item after the eleventh in a stated order. | — |
| twice | Two times. | — |
| two | The number 2. | — |
| type | A stated class of data or of thing. | — |
| unanswered | With no answer given. | — |
| unavailable | Not present for a stated use. | — |
| unbounded | With no stated bound. | — |
| unchanged | Not changed. | — |
| unclassifiable | Not able to be placed in a stated class. | — |
| unconverted | Not yet changed from a record into its stated final form. | — |
| undecided | With no ruling yet given. | — |
| undefined | With no stated definition. | — |
| under | Below, or subject to a rule. | — |
| understand | To know the meaning of a thing. | — |
| unfreed | Not released after a stated fetch, of a system object. | — |
| unguarded | Without the guard's protection. | — |
| unhardened | Not made harder to attack by the stated work. | — |
| unit | One stated item in a set or a measure. | — |
| unity | The state of being one stated whole. | — |
| universal | True for every case in a stated class. | — |
| universality | The quality of being true for every platform in the stated argument. | — |
| unix | Unix, an operating system family. | — |
| unjudged | Not yet examined by a stated check or review. | — |
| unless | Except in the case that. | — |
| unlock | To open a lock with a stated credential. | — |
| unmoved | Not changed in place, of a stated tree. | — |
| unnamed | With no stated name given. | — |
| unpack | To take the parts out of a package. | — |
| unpinned | Not held to a stated recorded value. | — |
| unplanned | With no cell in the stated plan. | — |
| unpriced | With no stated cost or breadth given. | — |
| unprotected | Without a stated protection. | — |
| unprotects | Removes a stated protection. | — |
| unproven | Not shown true by evidence yet. | — |
| unstated | Not stated in a stated record. | — |
| until | Up to a stated time. | — |
| up | Toward a higher place or state. | — |
| updated | Changed to the current form. | — |
| usability | The quality of a stated surface that lets a user do the stated work. | — |
| usage | The stated way a command is called, or the amount of use. | — |
| use | To employ a thing for its purpose. | utilize (for this) |
| user | A person who uses a machine or the guard. | — |
| utf | In artefact names: the Unicode text encoding family. | — |
| utf | UTF, the Unicode encoding family name in artefact text. | — |
| valid | In the required form, or true under a stated rule. | — |
| validation | The act of checking a stated thing against its form. | — |
| value | A stated amount, setting, or piece of data. | — |
| variable | A named value that can change, in code. | — |
| variance | The stated difference that a test measures. | — |
| variant | One stated form of a type or a case. | — |
| varies | Changes between cases. | — |
| variety | A range of different forms, in the rule against synonyms for variety. | — |
| vary | To change between stated cases. | — |
| verbatim | In the exact stated words or form. | — |
| verification | The act of checking a thing against its rule. | — |
| verified | Checked and found true. | — |
| verify | To check a thing against its rule. | — |
| version | One stated state of a product, with its number. | — |
| view | A presented form of data, or the act of seeing. | — |
| virtual | Made by software to act as a physical thing. | — |
| visibility | The stated right of a name to be seen from another module. | — |
| visible | Able to be seen. | — |
| visual | Of sight, in artefact names. | — |
| vocabulary | The set of approved words for a subject. | — |
| voice | The stated form of a sentence: active or passive. | — |
| void | To make a stated claim have no force. | — |
| wait | To stay until a stated event. | — |
| waitable | Able to be waited on by a stated call, of a system object. | — |
| wake | To make a stated process run again for a stated signal. | — |
| wall | In the phrase wall clock: real passing time. | — |
| warmup | The stated first runs of a campaign that do not count. | — |
| warning | A statement that names a risk. | — |
| warrant | The stated ground that makes a stated act right. | — |
| wart | A stated small defect that a record names and keeps. | — |
| was | Past form of be. | — |
| watched | Under observation by a stated check. | — |
| watcher | A program or a part that observes a stated set. | — |
| watches | Observes a stated set. | — |
| way | A stated method or route. | — |
| weak | Not strong against a stated test. | — |
| weakened | Made weaker against a stated test. | — |
| weight | The burden of a stated thing, as its size. | — |
| welcome | Giving a stated greeting to new arrivals. | — |
| well | In a good or correct way. | — |
| were | Past plural form of be. | — |
| what | The thing that. | — |
| whatever | Any stated thing, no matter which. | — |
| when | At the time that. | — |
| where | At the place that. | — |
| whether | If it is the case that, or not. | — |
| which | The stated one from a set. | — |
| while | During the time that. | — |
| whitespace | Space and tab characters in a text. | — |
| who | The person that. | — |
| whoever | Any person that. | — |
| whole | The complete set. | — |
| whose | Belonging to whom. | — |
| why | For what reason. | — |
| wide | Great in stated breadth. | — |
| widen | To make a stated thing broader. | — |
| width | How wide a thing is. | — |
| wiki | The staged documentation set that ships in the wiki folder. | — |
| will | The future form: at a stated later time. | — |
| win | To prevail in a stated argument or test. | — |
| window | A rectangular screen area that holds one session or program. | — |
| windows | Windows, an operating system. | — |
| winrm | WinRM, a Windows remote service name in artefact text. | — |
| wire | To connect a part by stated configuration. | — |
| with | Together with, or by means of. | — |
| within | Inside a stated bound. | — |
| without | Not having. | — |
| wmi | WMI, a Windows management service name in artefact text. | — |
| word | A stated unit of language with one form. | — |
| work | An act or a product of effort. | — |
| workflow | A stated series of steps that a system runs. | — |
| world | The stated system or setting in which a thing stands. | — |
| worse | Less good against a stated measure. | — |
| would | Conditional form of will: in the stated case. | — |
| wrapped | Placed inside another command or value. | — |
| wraps | Places a thing inside another command or value. | — |
| write | To put words or data into a file or a stream. | — |
| written | Put into text. | — |
| wrong | Not correct. | — |
| wsl | WSL, the Windows subsystem for Linux, in artefact text. | — |
| yes | The affirmative answer. | — |
| you | The reader. | — |
| your | Belonging to the reader. | — |
| zero | The number 0. | — |
| abort | To stop a run at once as an infrastructure failure, with no result claimed. | — |
| absent | Not present at the place a read or a check names. | — |
| accessibility | The interactive fitness of a product, one class of dimension (8). | — |
| actual | The value or state that stands in fact, not the stated one alone. | — |
| actually | In fact, as the mechanism itself does. | — |
| adjacent | Standing next to a named thing, with nothing between. | — |
| admission | The pass before a run that verifies every named presence and identity. | — |
| admitted | Let in by an admission pass that verified the stated conditions. | — |
| adopt | To take a decided item into the product or the record as standing. | — |
| aggregate | The final combined statement of a run over all its stages. | — |
| anew | Again, as a new act of the same kind. | — |
| artifact | An object that a run or a package produces and keeps. | — |
| assign | To put a stated label or value on a thing by the stated act. | — |
| automate | To make an act run by a program, with no person acting in it. | — |
| await | To stand ready for a named later act or value. | — |
| backing | The live store that stands behind a value a read presents. | — |
| baseline | The unloaded side of a pricing pair or of a context pair. | — |
| basis | The stated ground that a claim or a verdict rests on. | — |
| behave | To act in the way a thing's construction makes it act. | — |
| beyond | Past a named point, set, or bound. | — |
| boolean | Of the two-value true/false type, in artefact and record text. | — |
| bracket | A stated pair of bounds that a value stands inside. | — |
| bullet | One item line of a list, with its headword or its first words. | — |
| bundle | The preserved set of competitor binaries and their run-manifest. | — |
| candidacy | The standing of a candidate that one settlement question admitted. | — |
| candidate | A capture held pending beside a slot's retained entry, on deferred terms. | — |
| capture | The single traversal a read performs over a value, with its tokens. | — |
| captureless | Standing with no capture taken, as the persistence path stands. | — |
| catch | To stop a wrong value or binary at a stated gate before it is used. | — |
| caught | Past form of catch: stopped at the stated gate before use. | — |
| charge | To assign a condition or a duty to a named case or step. | — |
| checkout | A copy of a repo tree taken at a stated state, in artefact text. | — |
| classification | The class that a check or a census assigns to a thing. | — |
| cli | CLI, the command-line interface short form, in artefact text. | — |
| companion | A value that stands beside another value and is held with it. | — |
| comparative | Judged against another arm or value, not against a constant alone. | — |
| compatibility | Fitness with other systems and forms, one class of dimension (7). | — |
| component | One named part of a stated whole, as one part of a dimension. | — |
| concrete | In the exact form a consumer can use, not a stated form alone. | — |
| concurrency | Many runs or acts standing in flight at the same time, dimension (4). | — |
| configure | To set the values a run reads, in the configuration that names them. | — |
| confirmation | A stated check that a performed act in fact stands as performed. | — |
| consistency | The state of two records that agree where they state the same thing. | — |
| construct | To build a value or a record in the form its rule states. | — |
| consult | To read a stated record or value as the ground of a decision. | — |
| consume | To use up a value or a traversal so that it cannot be used again. | — |
| continue | To go on to the next stated stage or step after a stated event. | — |
| contradict | To state the opposite of a stated record, so the two cannot both stand. | — |
| coordinate | One stated position in a record that a check can name and recompute. | — |
| correctness | The state of being correct against the stated standard, dimension (1). | — |
| counterexample | One case that proves a claim false by standing against it. | — |
| custom | Made by the owner for one use, not a standard form. | — |
| defer | To hold a verdict for a later read that can decide it. | — |
| degrade | To make a verification weaker in a named way, never silently. | — |
| delegate | To hand an act to another object that performs it in the same shape. | — |
| deliver | To hand a stated thing over in its stated form, as a package is delivered. | — |
| derive | To reach a value or a class by a stated chain from its source. | — |
| detached | Separated, for the DETACHED console context of a run. | — |
| deterministic | Always giving the same result from the same stated inputs. | — |
| digest | To take the checksum of a stream or a file as its record. | — |
| disagreement | A stated difference between a value and the source it is held against. | — |
| dispersion | The spread of the paired differences of a price, in its stated form. | — |
| displace | To take the place of a standing thing, which then no longer stands. | — |
| distinction | The stated difference that keeps two kinds of thing apart. | — |
| dominate | To stand over every other stated case, so its state decides the whole. | — |
| drain | To empty a source by taking values from it until none are left. | — |
| driver | The program that runs one case or one family and records its evidence. | — |
| elapsed | Passed, for a stated time between two named points of a run. | — |
| element | One member of a stated set or one part of a stated structure. | — |
| elsewhere | At another stated place or on another stated machine. | — |
| emit | To put bytes or a record out on a stated stream or into a stated file. | — |
| enable | To make a stated act or path able to run, by a stated setting. | — |
| energy | The power a run uses, one cost class of dimension (3). | — |
| enforce | To hold a rule by a check that fails when the rule is breached. | — |
| evaluate | To judge a stated thing against its stated rule and record the state. | — |
| expensive | Costing much of a stated resource, as the priced class of items does. | — |
| expiry | The end of a thing's standing by time or by count alone. | — |
| factory | A mapping whose accesses produce the value they return over one source. | — |
| fallen | Past form of fall: moved down to a stated lower place or state. | — |
| fate | One of the stated ways a pending thing can end. | — |
| figure | A stated number that a check holds against its source. | — |
| flight | Used only in the compound pre-flight: the admission pass before a run. | — |
| formalize | To put a stated check or rule into its exact recorded form. | — |
| freeze | The campaign gate that asks a win or a tie on each gate dimension in each context. | — |
| further | To a greater distance in a stated chain, order, or argument. | — |
| gauntlet | The gauntlet, the working form of the guard binary, in artefact text. | — |
| good | Correct and fit against the stated standard, as a known-good record is. | — |
| grace | The stated wait after a deadline before a kill act, in a competitor's form. | — |
| grandchild | The process that a child process itself spawns, in a kill tree. | — |
| headroom | The stated space between a measured spread and the bound over it. | — |
| however | In whatever way or case, without exception for that case. | — |
| huge | Far beyond the size or value that the stated rule expects. | — |
| id | An identifier value, in artefact and record text. | — |
| identify | To name the exact thing a value or a class is, by its evidence. | — |
| include | To hold a thing inside a stated set, list, or total. | — |
| infer | To reach a value by reasoning from a stated value, not by reading it. | — |
| infra | Short form of infrastructure in the classification infra-failure, in artefact text. | — |
| infrastructure | The harness and machine support a run stands on, not the arm under test. | — |
| inherit | To receive a handle or a state from a parent process by the stated path. | — |
| insensitive | Not distinct by the stated distinction; for names, not distinct by letter case. | — |
| insert | To put a thing into a stated place inside a structure or a chain. | — |
| installation | The putting of a product onto a machine, one class of dimension (7). | — |
| instrumentation | The measuring that a runner performs on each call it makes. | — |
| intact | Whole and unchanged against the standard it is judged against. | — |
| integer | A whole number with no fraction, as a type a stamp can hold. | — |
| integrity | The state of data that stands whole and unchanged, dimension (5). | — |
| intend | To state a thing as the target or the plan of an act. | — |
| interim | The value a read serves while its verdict is deferred. | — |
| interleave | To run two stated sets in an alternating order, step by step. | — |
| interquartile | Between the first and third quartiles, for the range of a dispersion. | — |
| iterable | A value that a traversal can pass over, in the suite's mechanism. | — |
| iterator | The object that performs one traversal over a value. | — |
| laid | Past form of lay: put in place in the stated order or form. | — |
| lapse | To end by time or by count alone, with no deciding act. | — |
| latency | The time between a stated start and its stated end, in a run. | — |
| lay | To put a thing in a stated place or order. | — |
| length | The stated size of a stream, a file, or a text, in its unit. | — |
| likewise | In the same way, for the second of two stated cases alike. | — |
| mapping | A value that presents a stated value at each access a read makes. | — |
| mark | To name a thing as standing in a stated state, in its record. | — |
| materialization | The one read that turns a stated value into its concrete token list. | — |
| materialize | To perform the one read that turns a stated value into its token list. | — |
| maximum | The greatest value in a stated set of values. | — |
| memo | The one product a minting factory keeps and returns across accesses. | — |
| memoize | To keep the first product of an access and return that product again. | — |
| merely | Only, and nothing more than the stated thing. | — |
| metadata | Data that states the shape of a record or of a probe, not its values. | — |
| mirror | To copy a stated record into a second home in the same form. | — |
| mismatch | A stated difference between a value and the value its rule expects. | — |
| monetary | Of money, for the cost class of dimension (3). | — |
| moot | To make a pending question stop standing, by removing its ground. | — |
| ms | The millisecond unit short form, in artefact and record text. | — |
| namespace | The one folder of one campaign run, holding that run's records. | — |
| naturally | By the thing's own course, with no act of the harness on it. | — |
| neighbour | The field, label, container, element, or artifact adjacent to one already held. | — |
| newline | The one line-ending character a stream or a file adds at a line's end. | — |
| notwithstanding | Without exception for the stated thing, which does not change the rule. | — |
| observation | One recorded reading of a run, in the form its record states. | — |
| observe | To read and record a stated thing as evidence, in its stated form. | — |
| ok | The outcome word ok, in artefact and record text. | — |
| operational | Of the way a thing runs and is run, in its stated meanings. | — |
| operator | The person who runs a stated command or answers a stated prompt. | — |
| oppose | To stand against a stated thing, as a stated value stands against a reading. | — |
| override | A named value that wins over a default value for one run. | — |
| parity | The agreement of two stated counts or forms of the same checks. | — |
| passage | One complete going-over of a value by an iterator. | — |
| patch | To change a supplied binary or record in place, which the build never does. | — |
| pattern | A stated shape that bytes or values match or fail to match. | — |
| pend | To stand pending, with no decision taken yet. | — |
| pending | Held beside a standing thing, with no decision taken on it yet. | — |
| perform | To carry out a stated act, in full, as its rule states. | — |
| permit | To allow a stated act or form under a stated rule. | — |
| persistence | The standing of an object across following reads that return it. | — |
| pid | A process identifier value, in artefact and record text. | — |
| pool | To join stated values into one set, which the report never does across contexts. | — |
| port | A Windows port object, or its handle, in suite and artefact text. | — |
| portability | Fitness to move to another system, one class of dimension (7). | — |
| previously | At an earlier stated point in the same chain or record. | — |
| prior | Standing before the stated point in the same chain or record. | — |
| probe | A program or a step that reads a named fact and records it. | — |
| processkit | ProcessKit CLI, the competitor program, in artefact text. | — |
| produce | To make a value at an access, over a source, rather than retrieve it. | — |
| profile | The eight-dimension evaluation set of SPEC_Benchmark, with a state per dimension. | — |
| prolonged | Run for a long stated time, for the operation class of dimension (5). | — |
| propagate | To carry a value or a code onward through a stated chain unchanged. | — |
| provisional | Stated before the final act, and updated as that act completes. | — |
| qualify | To meet the stated conditions that let an entry count in a rule. | — |
| quality | The fitness of output against its standard, one class of dimension (1). | — |
| quantity | A stated amount that a check can hold and recompute. | — |
| rather | In place of the other stated case, or by the stated preference. | — |
| readable | Able to be read again by every consumer, in the stated form. | — |
| reading | One recorded value from a probe or a decider, in its stated form. | — |
| recomputation | The discipline of deriving a stated figure again from its source. | — |
| reconstruct | To build a value again from a stated form, in place of reading it. | — |
| recount | To count a stated total again from its source, as the census does. | — |
| recreate | To make a thing again in the same form, by a new act. | — |
| redirect | To send a stream to a stated file or device in place of its default. | — |
| refute | To prove a case or a claim false by its executed evidence. | — |
| regular | Of the ordinary stated form, as a regular file is an ordinary file. | — |
| reliability | The state of running without failure over time, dimension (5). | — |
| remainder | The part of a stated whole that a stated act does not cover. | — |
| remedy | The stated act that cures a named missing or wrong condition. | — |
| rep | One repetition of a slot in a campaign run, counted by the plan. | — |
| reportable | Able to be reported from its records, from the moment they exist. | — |
| resistance | The state of standing against a stated attack or input, dimension (6). | — |
| restart | To begin a traversal or a run again from its first point. | — |
| restatement | A stated figure or text that stands again in a second place. | — |
| retain | To keep a standing thing in its place across later acts. | — |
| retire | To end the standing of a thing by a stated act, in the open. | — |
| retrieve | To take a stored thing from the place it stands, unchanged. | — |
| retry | To attempt a stated act again after a stated failure. | — |
| rival | A competitor arm in a comparison, in the suite's text. | — |
| runner | The campaign program that executes one slot as one outer lifetime. | — |
| rustup | Rustup, the Rust toolchain installer, in artefact text. | — |
| scratch | A stated empty working folder made for one run or one shape. | — |
| seed | To set the recorded value that a stated order is shuffled under. | — |
| sense | One stated meaning of a word that carries more than one meaning. | — |
| settlement | The decision that a pending candidate is storing or is minting. | — |
| sha | SHA, the checksum family name, in artefact text as SHA-256. | — |
| shuffle | To put a stated order into a new order under a recorded seed. | — |
| site | One place in code or in a package where a stated act is performed. | — |
| skip | To not execute a test or a slot, with its reason recorded. | — |
| sleep | To wait without acting for a stated time, as a workload sleeps. | — |
| slot | One planned unit of a campaign run, of one kind, arm, path, and context. | — |
| spec | Short form of a specification's name, in artefact text as SPEC_Benchmark. | — |
| stamp | One of the values a construction puts on every record of its shape. | — |
| startup | The start of a program or a run, as one measured class of dimension (2). | — |
| stderr | The standard error stream, in artefact text. | — |
| stdin | The standard input stream, in artefact text. | — |
| stdout | The standard output stream, in artefact text. | — |
| storage | Data kept on a machine, as one cost class of dimension (3). | — |
| stub | A logic-exercise program of the suite that stands in for an arm or a probe. | — |
| succeed | To stand next after a stated thing, in its place, by the stated rule. | — |
| sum | To add stated values into one total. | — |
| survival | The state of a process that still lives after the act that ends it. | — |
| survivor | A process of a killed tree that still lives after the kill act. | — |
| sysprims | Sysprims, the competitor program, in artefact text. | — |
| systematic | Standing in every stated case of a class, not in one case alone. | — |
| template | A stated form with named places that a run fills with its values. | — |
| testkit | The test helper program of the guard tree, in artefact text. | — |
| throughput | The amount of stated work in a stated time, dimension (2). | — |
| tick | The smallest time step of the harness clock that a reading can state. | — |
| toolset | The stated set of compiler programs that a build uses, as the MSVC toolset. | — |
| totality | The rule that the holds and consumers never raise on the values they judge. | — |
| traversal | One passage over a value by an iterator, with its identity and reach. | — |
| unanchored | Standing with no verifiable anchor for a checker to read against. | — |
| uncertainty | The stated spread that stands beside a measured price or value. | — |
| unchecked | Not read by a check, as a survival that no probe answered for. | — |
| unconfirmed | Performed but not confirmed by the stated confirmation record. | — |
| undecidable | Not able to be decided by the evidence a mechanism may take. | — |
| unhashable | Not able to be hashed, as a value a consumer cannot key on. | — |
| unicode | Unicode, the character standard, in artefact text. | — |
| unimplemented | Not built, as a stated step or branch that stands named and unbuilt. | — |
| unmarked | Standing with no mark recorded on it, in the stated sense. | — |
| unmet | Not met, as a gate whose stated condition did not stand. | — |
| unremarked | Passed with no remark recorded, where a remark was owed. | — |
| unsettled | Not decided by a settlement, and no longer pending a verdict. | — |
| unverifiable | Named as not able to be verified, in the open, never silently passed. | — |
| verifiable | Able to be verified against a stated anchor or source. | — |
| very | The exact one stated, as the entry's very object is the exact object. | — |
| veto | A stated lack or failure that blocks a whole result by itself. | — |
| violation | A stated breach of a rule that a check names, in place of passing it. | — |
| vswhere | Vswhere, the Visual Studio locator program, in artefact text. | — |
| wherever | At every place that the stated condition stands. | — |
| yield | To give a token or a value at a step of a traversal or an access. | — |
| classified | Assigned to a stated class by its evidence, as a slot record is. | — |
| evaluation | The judging of a stated thing against its stated rule, with its state. | — |
| flow | A stated path of acts or values from its start to its end. | — |
| ordinary | Of the common stated form, as the ordinary factory is the common form. | — |
| overwrite | To write over a stated record in its own place, ending the first form. | — |
| pathext | PATHEXT, the Windows path-extension variable, in artefact text. | — |
| supplied | Past form of supply: given as a stated input to a run or a build. | — |
| unexplained | With no stated reason in the record that carries the thing. | — |
| withheld | Held back and not given to the check or case that names it. | — |
| wrote | Past form of write: put into a file or a stream at a stated time. | — |
| header | The first stated part of a file, a record, or an impl block. | — |
| implementation | One stated form that carries out a rule, a trait, or a design. | — |
| struct | A stated data structure of the backend, with its named fields. | — |
| aliveness | The state of a process that still lives, as a poll reads it. | — |
| attach | To join a stated channel or structure to a stated thing. | — |
| connect | To join one stated endpoint to another so that data can pass. | — |
| duplication | The making of a second copy of a stated thing, as a stated cost. | — |
| examine | To read a stated thing closely against its stated rule. | — |
| exchange | A stated passing of data in both directions over one channel. | — |
| invocation | One single calling of a program or of a function, as a count unit. | — |
| least | The smallest in a stated set, by the stated measure. | — |
| loop | A stated set of steps that runs again until its condition ends it. | — |
| overlapped | Standing over the same stated span as another act, in the overlapped form. | — |
| owe | To still have a stated act or result to give, as a campaign is owed. | — |
| poll | One reading of a stated state that may have changed, as a wait polls. | — |
| post | To put a stated result or value into its stated place for a reader. | — |
| reset | To put a stated value or event back to its first stated state. | — |
| resume | To go on with a stated act from the point where it stood. | — |
| setup | The stated acts that stand in place before a stated main act. | — |
| stack | The stated memory form that holds a call's own values, as stack storage. | — |
| stop | The stop event of an extendable run, one of its two named events. | — |
| walk | To pass over every member of a stated set in its stated order. | — |

[↑ Back to Contents](#contents)

## Style rules

Use approved words only. Use one word for one meaning. Do not use a different word for variety. Use the same term for the same thing in every page. Write a sentence with at most 20 words. Give one instruction in one instruction sentence. Use active voice for an instruction. Write a procedure as steps. Give one action in each step. Name a thing before a pronoun crosses a section boundary for that thing.

[↑ Back to Contents](#contents)

## Link colors

Internal links go to a section on the same page. External links go to a different page or file. The convention states internal links in red and external links in blue. The specification stands in the wiki set at `wiki/link-colors.css`. GitHub removes inline styles on its pages. The convention therefore does not apply on github.com today. The set states the convention honestly and does not fake it.

[↑ Back to Contents](#contents)
