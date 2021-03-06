mod builder;
mod interface;
pub mod intents;

use std::collections::HashMap;
use std::borrow::Cow::{Borrowed, self};

use interface::{Node, Predicate, NodeIndex, NodeData, predicates, Note};

use Reference::{Start, Current, End};

// re exports
pub use builder::{QPaperBuilder, Builder};
pub use intents::{Read, Write, Reference, Intent, Reader, Writer, WriteResult, ReadResult, IntentResult, MetaIntent};

#[derive(Debug, Clone)]
pub struct QuestionPaper {
    pub nodes: Vec<Node>,
    prev_index: usize,
    last_index: usize,
    total_questions: u32,
    marked: HashMap<usize, NodeData>,
    skipped: HashMap<usize, NodeData>,
    notes: Vec<Note>
}



impl QuestionPaper {
    pub fn new(nodes: Vec<Node>, last_index: usize, total_questions: u32) -> Self {
        QuestionPaper {
            nodes,
            prev_index:0,
            last_index,
            total_questions,
            marked: HashMap::new(),
            skipped: HashMap::new(),
            notes: vec![]
        }
    }

    // find a node on a certain predicate
    fn find<P: Predicate>(&self, predicate: P, next: usize, skip: usize) -> Find<P> {
        Find {
            question_paper: self,
            predicate,
            next,
            skip: skip -1
        }
    }

    // return the nth node in this document
    pub fn nth(&self, index: usize) -> Option<NodeIndex> {
        NodeIndex::new(self, index)
    }

    // get the total number of nodes
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    // get the previous index
    pub fn prev_index(&self) -> usize {
        self.prev_index
    }

    pub fn last_index(&self) -> usize {
        self.last_index 
    }

    pub fn update_previous(&mut self, index: usize) {
        self.prev_index = index;
    }

    // resolve a read or write intent
    pub fn resolve_intent(&mut self, intent: Intent) ->  IntentResult {
        match intent {
            Intent::ReadIntent(ref read_intent) => {
                let result = match self.resolve_read_intent(read_intent) {
                    Ok(node) => {
                        let index = node.index;
        
                        self.update_previous(index);
        
                        Ok(node.data.clone())
                    },
                    Err(e) => Err(e)
                };

                IntentResult::Read(result)
            },
            Intent::WriteIntent(ref write_intent) => {
                let result = self.resolve_write_intent(write_intent);

                IntentResult::Write(result)
            },
            Intent::Meta(ref meta) => {
                match meta {
                    MetaIntent::Marked => {
                        // get the total marked and read them out
                        let total_marked = self.marked.len();

                        return IntentResult::Meta(format!("You have {} marked questions", total_marked));
                    },
                    MetaIntent::Skipped => {
                        return IntentResult::Meta(format!("You have skipped {} question", self.skipped.len()));
                    }
                }
            }
        }
        
    }

    /// Check how many questions have been marked for review
    pub fn num_marked(&self) -> usize {
        self.marked.len()
    }

    pub fn total_questions(&self) -> u32 {
        self.total_questions
    }

    pub fn num_skipped(&self) -> usize {
        self.skipped.len()
    }

    /// get the notes
    pub fn notes(&self) -> &Vec<Note> {
        &self.notes
    }

    // resolve the read intent in 
    fn find_node(&mut self, reads: &Vec<Read>) -> ReadResult {
        let mut node = None;

        for intent in reads {
            node = Some(self.resolve_read_intent(intent));
        }

        if node.is_none(){
            return Err(Borrowed("Could not find the specified request"));
        }

        node.unwrap()
    }

}

impl Reader for QuestionPaper {
    /// Resolves a read intent
    fn resolve_read_intent(&mut self, read_intent: &Read) -> ReadResult {
        match read_intent {
            Read::Question(ref question) => self.resolve_question(question),
            Read::Section(ref section) => self.resolve_section(section),
        }
    }

    /// Resolve a question
    fn resolve_question(&mut self, reference: &Reference) -> ReadResult {
        let predicate = predicates::QuestionPredicate;

        self.resolve_referece(reference, predicate)
    }

    /// Resolve a section
    fn resolve_section(&mut self, reference: &Reference) -> ReadResult {
        let predicate = predicates::SectionPredicate;

        self.resolve_referece(reference, predicate)
    }

    /// Resolve from a reference
    fn resolve_referece<P: Predicate>(&mut self, reference: &Reference, predicate: P) -> ReadResult {

        let (prev, skip) = match reference {
            Start(skip) => (0, skip.abs() as usize),
            Current(skip) => (self.prev_index(), (skip.abs() as usize) + 1),
            End(skip) => (self.last_index(), skip.abs() as usize)
            
        };
        

        self.resolve(predicate, prev, skip, reference)
    }

    fn resolve<P: Predicate>(&mut self, predicate: P, prev: usize, skip: usize, reference: &Reference) -> ReadResult {
        let finder = self.find(predicate, prev, skip);


        if reference.is_forward(){
            self.find_next(finder)
        }else{
            self.find_back(finder)
        }
    }

    /// Do a foward find
    fn find_next<P: Predicate>(&self, mut finder: Find<P>) -> ReadResult {
        if let Some(node) = finder.next(){
            Ok(node.raw().clone())
        }else{
            Err(Borrowed("Could not find a next node"))
        }
    }

    /// Do a reverse find
    fn find_back<P: Predicate>(&self, mut finder: Find<P>) -> ReadResult {
        if let Some(node) = finder.next_back(){
            Ok(node.raw().clone())
        }else{
            Err(Borrowed("Could not resolve a previous node"))
        }
    }

}

impl Writer for QuestionPaper {
    /// Resolve a write intent
    fn resolve_write_intent(&mut self, write_intent: &Write) ->  WriteResult{
        match write_intent {
            Write::Mark(ref read_intents) => return self.mark_for_review(read_intents),
            Write::Skip(ref read_intents) => self.skip(read_intents),
            Write::Note(ref read_intents, note) => self.note(read_intents, note.to_string())
        }
    }


    // process a read intent and mark it for review
    fn mark_for_review(&mut self, reads: &Vec<Read>) -> WriteResult {
        if let Ok(node) = self.find_node(reads) {
            self.marked.insert(node.index, node.data.clone());
            

            return WriteResult::Success("Question has been marked for review".to_string());
        }
        
        return WriteResult::Error(Borrowed("Could not mark the specified item for review. Please try again"));        
    }

    fn skip(&mut self, reads: &Vec<Read>) -> WriteResult {
        if let Ok(node) = self.find_node(reads) {
            self.skipped.insert(node.index, node.data.clone());

            return WriteResult::Success("Question has been skipped".to_string());
        }
        
        return WriteResult::Error(Borrowed("Could not skip the specified item. Please try again"));        
    }

    /// Take a note on this node
    fn note(&mut self, reads: &Vec<Read>, note: String) -> WriteResult {
        if let Ok(node) = self.find_node(reads){
            self.notes.push(Note {
                note,
                index: node.index
            });
           
            return WriteResult::Success("A not has been taken".to_string());
        }

        return WriteResult::Error(Borrowed("Could not take a note as requested"));

        
    }
}


pub struct Find<'a, P:Predicate> {
    predicate: P,
    next: usize,
    question_paper: &'a QuestionPaper,
    skip: usize
}

impl <'a, P: Predicate> Iterator for Find<'a, P> {
    type Item = NodeIndex<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.next < self.question_paper.len(){
            let node = self.question_paper.nth(self.next).unwrap();

            self.next += 1;

            if self.predicate.matches(&node){
                if self.skip >= 1 {  
                   
                    self.skip -= 1;
                }else{
                    return Some(node);
                }
            }
        }

        None
    }
}


impl<'a, P: Predicate> DoubleEndedIterator for Find<'a, P> {
    fn next_back(&mut self) -> Option<NodeIndex<'a>> {
        while self.next > 0 {
            let node = self.question_paper.nth(self.next).unwrap();

            self.next -= 1;

            if self.predicate.matches(&node) {
                if self.skip >= 1 {
                    self.skip -= 1;
                }else{
                    return Some(node);
                }
               
            }
        }

        None
    }
}