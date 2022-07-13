package main

import (
	"flag"
	"fmt"
	"net/http"
)

var port int

func init() {
	flag.IntVar(&port, "port", 8080, "the port to run the server on")
	flag.Parse()
}

func main() {
	http.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		fmt.Printf("%+v\n", r)
		w.WriteHeader(http.StatusOK)
	})
	http.HandleFunc("/api", func(w http.ResponseWriter, r *http.Request) {
		fmt.Printf("%+v\n", r)
		w.WriteHeader(http.StatusOK)
		w.Write([]byte("{\"message\": \"hello from golang service\"}"))
	})

	fmt.Printf("starting server on port %d\n", port)
	panic(http.ListenAndServe(fmt.Sprintf(":%d", port), nil))
}
