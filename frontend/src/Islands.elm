module Islands exposing (main)

import Browser
import Browser.Events
import Html exposing (Html, button, div, footer, form, h2, header, img, input, label, li, nav, option, p, select, small, span, strong, text, ul)
import Html.Attributes exposing (alt, attribute, checked, class, height, href, id, name, placeholder, selected, src, type_, value, width)
import Html.Events exposing (on, onClick, onInput, onSubmit, preventDefaultOn, stopPropagationOn)
import Http
import Json.Decode as Decode
import Json.Encode as Encode


type alias Flags =
    { island : String
    , props : Decode.Value
    }


type Model
    = SearchFormModel SearchForm
    | SearchFiltersModel SearchFilters
    | GalleryModel Gallery
    | InquiryModel Inquiry
    | MobileNavModel MobileNav
    | Unknown String


type alias SearchForm =
    { destinations : List Destination
    , destination : String
    , query : String
    , checkIn : String
    , checkOut : String
    , adults : Int
    , children : Int
    , compact : Bool
    , open : Maybe DateTarget
    , year : Int
    , month : Int
    }


type alias Destination =
    { slug : String
    , name : String
    }


type DateTarget
    = CheckIn
    | CheckOut


type alias SearchFilters =
    { destination : String
    , query : String
    , checkIn : String
    , checkOut : String
    , adults : Int
    , children : Int
    , minPrice : String
    , maxPrice : String
    , rating : String
    , amenities : List Amenity
    , selected : List String
    , offers : Bool
    , sort : String
    }


type alias Amenity =
    { id : String
    , name : String
    }


type alias Gallery =
    { name : String
    , items : List Photo
    , open : Bool
    , active : String
    , category : String
    , touch : Float
    }


type alias Photo =
    { id : String
    , src : String
    , alt : String
    , caption : String
    , category : String
    , webp : String
    , avif : String
    }


type alias Inquiry =
    { hotelId : String
    , hotelName : String
    , roomId : String
    , offerId : String
    , checkIn : String
    , checkOut : String
    , adults : String
    , children : String
    , guest : String
    , email : String
    , phone : String
    , message : String
    , website : String
    , pending : Bool
    , error : String
    , reference : String
    , open : Maybe DateTarget
    , year : Int
    , month : Int
    }


type alias MobileNav =
    { links : List Link
    , path : String
    , open : Bool
    }


type alias Link =
    { label : String
    , href : String
    , description : String
    }


type Msg
    = OpenDate DateTarget
    | CloseDate
    | KeepDate
    | ShiftMonth Int
    | ChooseDate DateTarget String
    | ToggleAmenity String
    | ToggleOffers
    | OpenPhoto String
    | CloseGallery
    | StepPhoto Int
    | FilterCategory String
    | TouchStart Float
    | TouchEnd Float
    | EditInquiry String String
    | SubmitInquiry
    | GotCsrf (Result Http.Error String)
    | Submitted (Result String String)
    | ToggleMenu


main : Program Flags Model Msg
main =
    Browser.element
        { init = init
        , view = view
        , update = update
        , subscriptions = subscriptions
        }


init : Flags -> ( Model, Cmd Msg )
init flags =
    ( case flags.island of
        "SearchForm" ->
            SearchFormModel (decodeSearch flags.props)

        "SearchExperience" ->
            SearchFiltersModel (decodeFilters flags.props)

        "Gallery" ->
            GalleryModel (decodeGallery flags.props)

        "InquiryForm" ->
            InquiryModel (decodeInquiry flags.props)

        "MobileNav" ->
            MobileNavModel (decodeNav flags.props)

        other ->
            Unknown other
    , Cmd.none
    )


update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case ( msg, model ) of
        ( OpenDate target, SearchFormModel form ) ->
            ( SearchFormModel { form | open = toggleDate form.open target }, Cmd.none )

        ( CloseDate, SearchFormModel form ) ->
            ( SearchFormModel { form | open = Nothing }, Cmd.none )

        ( ShiftMonth delta, SearchFormModel form ) ->
            ( SearchFormModel (shift form.year form.month delta |> \( y, m ) -> { form | year = y, month = m }), Cmd.none )

        ( ChooseDate CheckIn date, SearchFormModel form ) ->
            let
                checkOut =
                    if form.checkOut /= "" && date >= form.checkOut then
                        ""

                    else
                        form.checkOut
            in
            ( SearchFormModel { form | checkIn = date, checkOut = checkOut, open = Nothing }, Cmd.none )

        ( ChooseDate CheckOut date, SearchFormModel form ) ->
            ( SearchFormModel { form | checkOut = date, open = Nothing }, Cmd.none )

        ( OpenDate target, InquiryModel form ) ->
            ( InquiryModel { form | open = toggleDate form.open target }, Cmd.none )

        ( CloseDate, InquiryModel form ) ->
            ( InquiryModel { form | open = Nothing }, Cmd.none )

        ( ShiftMonth delta, InquiryModel form ) ->
            ( InquiryModel (shift form.year form.month delta |> \( y, m ) -> { form | year = y, month = m }), Cmd.none )

        ( ChooseDate CheckIn date, InquiryModel form ) ->
            ( InquiryModel { form | checkIn = date, open = Nothing }, Cmd.none )

        ( ChooseDate CheckOut date, InquiryModel form ) ->
            ( InquiryModel { form | checkOut = date, open = Nothing }, Cmd.none )

        ( ToggleAmenity id, SearchFiltersModel filters ) ->
            let
                selected =
                    if List.member id filters.selected then
                        List.filter ((/=) id) filters.selected

                    else
                        id :: filters.selected
            in
            ( SearchFiltersModel { filters | selected = selected }, Cmd.none )

        ( ToggleOffers, SearchFiltersModel filters ) ->
            ( SearchFiltersModel { filters | offers = not filters.offers }, Cmd.none )

        ( OpenPhoto id, GalleryModel gallery ) ->
            ( GalleryModel { gallery | open = True, active = id }, Cmd.none )

        ( CloseGallery, GalleryModel gallery ) ->
            ( GalleryModel { gallery | open = False }, Cmd.none )

        ( StepPhoto delta, GalleryModel gallery ) ->
            ( GalleryModel { gallery | active = neighbor (visiblePhotos gallery) gallery.active delta }, Cmd.none )

        ( FilterCategory name, GalleryModel gallery ) ->
            ( GalleryModel { gallery | category = name, active = firstPhotoId gallery.items name }, Cmd.none )

        ( TouchStart x, GalleryModel gallery ) ->
            ( GalleryModel { gallery | touch = x }, Cmd.none )

        ( TouchEnd x, GalleryModel gallery ) ->
            let
                delta =
                    gallery.touch - x
            in
            if abs delta >= 45 then
                ( GalleryModel { gallery | active = neighbor (visiblePhotos gallery) gallery.active (if delta > 0 then 1 else -1) }, Cmd.none )

            else
                ( model, Cmd.none )

        ( EditInquiry field next, InquiryModel form ) ->
            ( InquiryModel (setInquiry field next form), Cmd.none )

        ( SubmitInquiry, InquiryModel form ) ->
            if form.pending then
                ( model, Cmd.none )

            else if form.checkIn == "" || form.checkOut == "" || form.checkOut <= form.checkIn then
                ( InquiryModel { form | error = "Choose a check-out date after check-in." }, Cmd.none )

            else
                ( InquiryModel { form | pending = True, error = "" }, fetchCsrf )

        ( GotCsrf (Ok token), InquiryModel form ) ->
            ( model, sendInquiry token form )

        ( GotCsrf (Err _), InquiryModel form ) ->
            ( InquiryModel { form | pending = False, error = "Unable to prepare your request. Please try again." }, Cmd.none )

        ( Submitted (Ok reference), InquiryModel form ) ->
            ( InquiryModel { form | pending = False, reference = reference }, Cmd.none )

        ( Submitted (Err reason), InquiryModel form ) ->
            ( InquiryModel { form | pending = False, error = reason }, Cmd.none )

        ( ToggleMenu, MobileNavModel nav ) ->
            ( MobileNavModel { nav | open = not nav.open }, Cmd.none )

        ( KeepDate, _ ) ->
            ( model, Cmd.none )

        _ ->
            ( model, Cmd.none )


subscriptions : Model -> Sub Msg
subscriptions model =
    Sub.batch [ dateSubscription model, gallerySubscription model ]


dateSubscription : Model -> Sub Msg
dateSubscription model =
    if dateOpen model then
        Sub.batch
            [ Browser.Events.onMouseDown (Decode.succeed CloseDate)
            , Browser.Events.onKeyDown
                (Decode.field "key" Decode.string
                    |> Decode.andThen
                        (\key ->
                            if key == "Escape" then
                                Decode.succeed CloseDate

                            else
                                Decode.fail "not escape"
                        )
                )
            ]

    else
        Sub.none


gallerySubscription : Model -> Sub Msg
gallerySubscription model =
    case model of
        GalleryModel gallery ->
            if gallery.open then
                Browser.Events.onKeyDown
                    (Decode.field "key" Decode.string
                        |> Decode.andThen
                            (\key ->
                                case key of
                                    "ArrowLeft" ->
                                        Decode.succeed (StepPhoto -1)

                                    "ArrowRight" ->
                                        Decode.succeed (StepPhoto 1)

                                    "Escape" ->
                                        Decode.succeed CloseGallery

                                    _ ->
                                        Decode.fail "not a gallery key"
                            )
                    )

            else
                Sub.none

        _ ->
            Sub.none


dateOpen : Model -> Bool
dateOpen model =
    case model of
        SearchFormModel form ->
            form.open /= Nothing

        InquiryModel form ->
            form.open /= Nothing

        _ ->
            False


toggleDate : Maybe DateTarget -> DateTarget -> Maybe DateTarget
toggleDate open target =
    if open == Just target then
        Nothing

    else
        Just target


holdDate : Html.Attribute Msg
holdDate =
    stopPropagationOn "mousedown" (Decode.succeed ( KeepDate, True ))


openClass : Maybe DateTarget -> DateTarget -> String
openClass open target =
    if open == Just target then
        " date-picker--open"

    else
        ""


view : Model -> Html Msg
view model =
    case model of
        SearchFormModel form ->
            searchForm form

        SearchFiltersModel filters ->
            searchFilters filters

        GalleryModel gallery ->
            galleryView gallery

        InquiryModel form ->
            inquiryView form

        MobileNavModel nav ->
            mobileNav nav

        Unknown name ->
            p [] [ text ("Unknown island " ++ name) ]


searchForm : SearchForm -> Html Msg
searchForm form =
    Html.form
        [ class
            (if form.compact then
                "search-form search-form--compact"

             else
                "search-form"
            )
        , Html.Attributes.action "/search"
        , Html.Attributes.method "get"
        , attribute "aria-label" "Search hotels"
        ]
        [ formField "search-destination" "Destination" <|
            div [ class "select" ]
                [ select [ id "search-destination", name "destination" ]
                    (option [ value "", selected (form.destination == "") ] [ text "Anywhere" ]
                        :: List.map
                            (\destination ->
                                option [ value destination.slug, selected (form.destination == destination.slug) ] [ text destination.name ]
                            )
                            form.destinations
                    )
                ]
        , dateField "search-check-in" "checkIn" "Check in" CheckIn form.checkIn form
        , dateField "search-check-out" "checkOut" "Check out" CheckOut form.checkOut form
        , formField "search-adults" "Guests" <|
            div [ class "select" ]
                [ select [ id "search-adults", name "adults" ]
                    (List.range 1 12
                        |> List.map
                            (\count ->
                                option [ value (String.fromInt count), selected (count == form.adults) ]
                                    [ text (String.fromInt count ++ " adult" ++ plural count) ]
                            )
                    )
                ]
        , input [ type_ "hidden", name "children", value (String.fromInt form.children) ] []
        , input [ type_ "hidden", name "query", value form.query ] []
        , input [ type_ "hidden", name "sort", value "relevance" ] []
        , input [ type_ "hidden", name "page", value "1" ] []
        , button [ type_ "submit", class "button is-primary search-form__submit", attribute "aria-label" "Search hotels" ]
            [ span [] [ text "Search" ] ]
        ]


dateField : String -> String -> String -> DateTarget -> String -> SearchForm -> Html Msg
dateField fieldId fieldName labelText target current form =
    div [ class ("search-form__field date-picker" ++ openClass form.open target), holdDate ]
        [ label [ class "label", Html.Attributes.for fieldId ] [ text labelText ]
        , input [ type_ "hidden", name fieldName, value current ] []
        , button [ type_ "button", class "input date-picker__trigger", id fieldId, onClick (OpenDate target) ]
            [ text
                (if current == "" then
                    "Add date"

                 else
                    current
                )
            ]
        , if form.open == Just target then
            div [ class "date-picker__popover" ] [ calendar form.year form.month target ]

          else
            text ""
        ]


searchFilters : SearchFilters -> Html Msg
searchFilters filters =
    Html.aside [ class "card search-page__filters" ]
        [ Html.header [ class "search-page__filters-header" ]
            [ h2 [ class "search-page__filters-title" ] [ text "Filters" ] ]
        , div [ class "card-content" ]
            [ Html.form [ Html.Attributes.action "/search", Html.Attributes.method "get", attribute "aria-label" "Filter hotels" ]
                [ hidden "destination" filters.destination
                , hidden "checkIn" filters.checkIn
                , hidden "checkOut" filters.checkOut
                , hidden "adults" (String.fromInt filters.adults)
                , hidden "children" (String.fromInt filters.children)
                , hidden "sort" filters.sort
                , hidden "page" "1"
                , labeled "hotel-query" "Search by name" <|
                    input [ class "input", id "hotel-query", name "query", value filters.query ] []
                , labeled "min-price" "Minimum price" <|
                    input [ class "input", id "min-price", name "minPrice", type_ "number", value filters.minPrice ] []
                , labeled "max-price" "Maximum price" <|
                    input [ class "input", id "max-price", name "maxPrice", type_ "number", value filters.maxPrice ] []
                , labeled "min-rating" "Minimum rating" <|
                    div [ class "select" ]
                        [ select [ id "min-rating", name "rating" ]
                            (option [ attribute "value" "", selected (filters.rating == "") ] [ text "Any" ]
                                :: List.map
                                    (\score ->
                                        option [ value score, selected (filters.rating == score) ] [ text (score ++ "+") ]
                                    )
                                    [ "3", "3.5", "4", "4.5" ]
                            )
                        ]
                , div [ class "field" ]
                    [ p [ class "label" ] [ text "Amenities" ]
                    , ul []
                        (List.map
                            (\amenity ->
                                li []
                                    [ label []
                                        [ input
                                            [ type_ "checkbox"
                                            , name "amenities"
                                            , value amenity.id
                                            , checked (List.member amenity.id filters.selected)
                                            , onClick (ToggleAmenity amenity.id)
                                            ]
                                            []
                                        , text amenity.name
                                        ]
                                    ]
                            )
                            filters.amenities
                        )
                    ]
                , label [ class "checkbox" ]
                    [ input [ type_ "checkbox", name "offers", value "true", checked filters.offers, onClick ToggleOffers ] []
                    , text " Offers available"
                    ]
                , button [ class "button is-primary", type_ "submit" ] [ text "Apply filters" ]
                ]
            ]
        ]


galleryImage : Photo -> String -> Bool -> Html msg
galleryImage photo sizes primary =
    Html.node "picture"
        []
        (List.filterMap identity
            [ imageSource "image/avif" photo.avif sizes
            , imageSource "image/webp" photo.webp sizes
            ]
            ++ [ img
                    [ class "gallery-grid__image"
                    , src photo.src
                    , alt photo.alt
                    , attribute "sizes" sizes
                    , width 1280
                    , height 800
                    , attribute "loading"
                        (if primary then
                            "eager"

                         else
                            "lazy"
                        )
                    ]
                    []
               ]
        )


imageSource : String -> String -> String -> Maybe (Html msg)
imageSource mime srcset sizes =
    if srcset == "" then
        Nothing

    else
        Just (Html.node "source" [ attribute "type" mime, attribute "srcset" srcset, attribute "sizes" sizes ] [])


galleryView : Gallery -> Html Msg
galleryView gallery =
    if List.isEmpty gallery.items then
        div [] []

    else
        div []
            [ div [ class "gallery-grid" ]
                ((List.take 5 gallery.items
                    |> List.indexedMap
                        (\index photo ->
                            button
                                [ type_ "button"
                                , class ("gallery-grid__item" ++ (if index == 0 then " gallery-grid__item--primary" else "") ++ (if index == 3 then " gallery-grid__item--fourth" else "") ++ (if index == 4 then " gallery-grid__item--fifth" else ""))
                                , attribute "aria-label" ("Open " ++ gallery.name ++ " gallery image " ++ String.fromInt (index + 1))
                                , onClick (OpenPhoto photo.id)
                                ]
                                [ span [ class "gallery-grid__image-wrap" ]
                                    [ galleryImage photo (if index == 0 then "(max-width: 640px) 66vw, calc((min(100vw, 73.75rem) - 2.5rem) / 2)" else "(max-width: 640px) 34vw, calc((min(100vw, 73.75rem) - 2.5rem) / 4)") (index == 0) ]
                                , if index > 0 then
                                    span [ class "gallery-grid__category" ] [ text photo.category ]

                                  else
                                    text ""
                                ]
                        )
                 )
                    ++ [ button [ type_ "button", class "button is-secondary gallery-grid__all", onClick (OpenPhoto (activeId gallery)) ]
                            [ text ("View all " ++ String.fromInt (List.length gallery.items) ++ " photos") ]
                       ]
                )
            , if gallery.open then
                lightbox gallery

              else
                text ""
            ]


lightbox : Gallery -> Html Msg
lightbox gallery =
    let
        photos =
            visiblePhotos gallery

        index =
            photoIndex photos gallery.active

        photo =
            List.drop index photos
                |> List.head
                |> Maybe.withDefault { id = "", src = "", alt = "", caption = "", category = "", webp = "", avif = "" }
    in
    Html.node "dialog"
        [ class "modal gallery-lightbox"
        , attribute "aria-label" (gallery.name ++ " image gallery")
        , preventDefaultOn "cancel" (Decode.succeed ( CloseGallery, True ))
        , on "click" backdropClick
        ]
        [ div [ class "modal-content gallery-lightbox__content" ]
            [ header [ class "gallery-lightbox__header" ]
                [ div [ class "gallery-lightbox__heading" ]
                    [ strong [] [ text gallery.name ]
                    , small [] [ text (String.fromInt (index + 1) ++ " of " ++ String.fromInt (List.length photos)) ]
                    ]
                , button [ type_ "button", class "button is-ghost gallery-lightbox__close button-icon", attribute "aria-label" "Close gallery", onClick CloseGallery ]
                    [ glyph "×" ]
                ]
            , nav [ class "gallery-lightbox__categories", attribute "aria-label" "Photo categories" ]
                (List.map (categoryButton gallery.category) (categoryNames gallery.items))
            , div
                [ class "gallery-lightbox__stage"
                , on "touchstart" (Decode.field "touches" (Decode.index 0 (Decode.field "clientX" Decode.float)) |> Decode.map TouchStart)
                , on "touchend" (Decode.field "changedTouches" (Decode.index 0 (Decode.field "clientX" Decode.float)) |> Decode.map TouchEnd)
                ]
                (img [ class "gallery-lightbox__active-image", src photo.src, alt photo.alt, width 1920, height 1080 ] []
                    :: (if List.length photos > 1 then
                            [ button [ type_ "button", class "button is-ghost gallery-lightbox__previous button-icon-lg", attribute "aria-label" "Previous photo", onClick (StepPhoto -1) ]
                                [ glyph "‹" ]
                            , button [ type_ "button", class "button is-ghost gallery-lightbox__next button-icon-lg", attribute "aria-label" "Next photo", onClick (StepPhoto 1) ]
                                [ glyph "›" ]
                            ]

                        else
                            []
                       )
                )
            , footer [ class "gallery-lightbox__footer" ]
                [ div [ class "gallery-lightbox__caption" ]
                    [ span [] [ text photo.caption ]
                    , span [] [ text photo.category ]
                    ]
                , div [ class "gallery-lightbox__thumbnails" ]
                    (List.map (thumbnail gallery.active) photos)
                ]
            ]
        ]


categoryButton : String -> String -> Html Msg
categoryButton current name =
    button
        [ type_ "button"
        , class ("button is-ghost is-small gallery-lightbox__category" ++ (if name == current then " gallery-lightbox__category--active" else ""))
        , attribute "aria-pressed" (if name == current then "true" else "false")
        , onClick (FilterCategory name)
        ]
        [ text name ]


thumbnail : String -> Photo -> Html Msg
thumbnail active photo =
    let
        label =
            if photo.caption == "" then
                photo.alt

            else
                photo.caption

        current =
            if photo.id == active then
                [ attribute "aria-current" "true" ]

            else
                []
    in
    button
        ([ type_ "button"
         , class ("gallery-lightbox__thumbnail" ++ (if photo.id == active then " gallery-lightbox__thumbnail--active" else ""))
         , attribute "aria-label" ("View " ++ label)
         , onClick (OpenPhoto photo.id)
         ]
            ++ current
        )
        [ img [ src photo.src, alt "", width 192, height 128 ] [] ]


glyph : String -> Html msg
glyph mark =
    span [ class "gallery-lightbox__glyph", attribute "aria-hidden" "true" ] [ text mark ]


backdropClick : Decode.Decoder Msg
backdropClick =
    backdropMsg CloseGallery


backdropMsg : Msg -> Decode.Decoder Msg
backdropMsg message =
    Decode.at [ "target", "tagName" ] Decode.string
        |> Decode.andThen
            (\tag ->
                if tag == "DIALOG" then
                    Decode.succeed message

                else
                    Decode.fail "inside the dialog"
            )




inquiryView : Inquiry -> Html Msg
inquiryView form =
    if form.reference /= "" then
        div [ class "notification is-success" ]
            [ h2 [] [ text "Inquiry received" ]
            , p [] [ text ("Reference " ++ form.reference) ]
            , p [] [ text "This is not a reservation; no payment has been taken." ]
            ]

    else
        Html.form [ class "inquiry-form", preventSubmit SubmitInquiry ]
            [ if form.error /= "" then
                p [ class "notification is-danger", attribute "role" "alert" ] [ text form.error ]

              else
                text ""
            , dateInquiry "inquiry-check-in" "Check in" CheckIn form.checkIn form
            , dateInquiry "inquiry-check-out" "Check out" CheckOut form.checkOut form
            , labeled "inquiry-adults" "Adults" <|
                input [ class "input", id "inquiry-adults", type_ "number", value form.adults, onInput (EditInquiry "adults") ] []
            , labeled "inquiry-children" "Children" <|
                input [ class "input", id "inquiry-children", type_ "number", value form.children, onInput (EditInquiry "children") ] []
            , labeled "inquiry-name" "Name" <|
                input [ class "input", id "inquiry-name", value form.guest, onInput (EditInquiry "name") ] []
            , labeled "inquiry-email" "Email" <|
                input [ class "input", id "inquiry-email", type_ "email", value form.email, onInput (EditInquiry "email") ] []
            , labeled "inquiry-phone" "Phone" <|
                input [ class "input", id "inquiry-phone", value form.phone, onInput (EditInquiry "phone") ] []
            , labeled "inquiry-message" "Message" <|
                Html.textarea [ class "textarea", id "inquiry-message", value form.message, onInput (EditInquiry "message") ] []
            , div [ attribute "hidden" "hidden" ]
                [ label [] [ text "Website" ]
                , input [ name "website", value form.website, onInput (EditInquiry "website"), attribute "tabindex" "-1", attribute "autocomplete" "off" ] []
                ]
            , button [ class "button is-primary", type_ "submit", Html.Attributes.disabled form.pending ]
                [ text
                    (if form.pending then
                        "Sending…"

                     else
                        "Send inquiry"
                    )
                ]
            , p [] [ text "No payment is taken. The hotel confirms availability personally." ]
            ]


dateInquiry : String -> String -> DateTarget -> String -> Inquiry -> Html Msg
dateInquiry fieldId labelText target current form =
    div [ class ("field date-picker" ++ openClass form.open target), holdDate ]
        [ label [ class "label", Html.Attributes.for fieldId ] [ text labelText ]
        , button [ type_ "button", class "input date-picker__trigger", id fieldId, onClick (OpenDate target) ]
            [ text
                (if current == "" then
                    "Add date"

                 else
                    current
                )
            ]
        , if form.open == Just target then
            div [ class "date-picker__popover" ] [ calendar form.year form.month target ]

          else
            text ""
        ]


mobileNav : MobileNav -> Html Msg
mobileNav nav =
    div []
        [ button
            [ type_ "button"
            , class "button is-ghost button-icon site-header__mobile-toggle"
            , attribute "aria-label" "Open navigation"
            , attribute "aria-expanded" (if nav.open then "true" else "false")
            , onClick ToggleMenu
            ]
            [ span [ class "site-header__menu-icon", attribute "aria-hidden" "true" ]
                [ span [] [], span [] [], span [] [] ]
            ]
        , if nav.open then
            mobileMenu nav

          else
            text ""
        ]


mobileMenu : MobileNav -> Html Msg
mobileMenu menu =
    Html.node "dialog"
        [ class "modal mobile-navigation"
        , attribute "aria-labelledby" "mobile-navigation-title"
        , attribute "aria-describedby" "mobile-navigation-description"
        , preventDefaultOn "cancel" (Decode.succeed ( ToggleMenu, True ))
        , on "click" (backdropMsg ToggleMenu)
        ]
        [ div [ class "modal-content mobile-navigation__content" ]
            [ header [ class "mobile-navigation__header" ]
                [ div []
                    [ h2 [ id "mobile-navigation-title" ]
                        [ Html.a [ href "/", class "brand" ]
                            [ span [ class "brand__name" ] [ text "Elsewhere" ]
                            , small [ class "brand__tagline" ] [ text "STAYS & STORIES" ]
                            ]
                        ]
                    , p [ id "mobile-navigation-description", class "mobile-navigation__description" ]
                        [ text "Independent hotels and slower journeys, selected with care." ]
                    ]
                , button [ type_ "button", class "button is-ghost button-icon mobile-navigation__close", attribute "aria-label" "Close navigation", onClick ToggleMenu ]
                    [ glyph "×" ]
                ]
            , div [ class "mobile-navigation__body" ]
                [ nav [ class "mobile-navigation__links", attribute "aria-label" "Mobile navigation" ]
                    (List.map (mobileLink menu.path) menu.links)
                , div [ class "mobile-navigation__footer" ]
                    [ Html.a [ class "button is-primary mobile-navigation__search", href "/search" ] [ text "Find a stay" ]
                    , p [] [ text "Search curated hotels by destination, style, and amenities." ]
                    ]
                ]
            ]
        ]


mobileLink : String -> Link -> Html Msg
mobileLink path link =
    let
        current =
            if path == link.href then
                [ attribute "aria-current" "page" ]

            else
                []
    in
    Html.a
        ([ href link.href, class "mobile-navigation__link" ] ++ current)
        [ span [ class "mobile-navigation__icon", attribute "aria-hidden" "true" ] [ glyph "›" ]
        , span []
            [ text link.label
            , if link.description == "" then
                text ""

              else
                span [] [ text link.description ]
            ]
        , span [ class "mobile-navigation__chevron", attribute "aria-hidden" "true" ] [ glyph "›" ]
        ]


calendar : Int -> Int -> DateTarget -> Html Msg
calendar year month target =
    div []
        [ div [ class "date-picker__nav" ]
            [ button [ type_ "button", attribute "aria-label" "Previous month", onClick (ShiftMonth -1) ] [ text "‹" ]
            , span [] [ text (monthName month ++ " " ++ String.fromInt year) ]
            , button [ type_ "button", attribute "aria-label" "Next month", onClick (ShiftMonth 1) ] [ text "›" ]
            ]
        , div [ class "date-picker__weekdays" ]
            (List.map (\name -> span [] [ text name ]) [ "Su", "Mo", "Tu", "We", "Th", "Fr", "Sa" ])
        , div [ class "date-picker__days" ]
            (List.repeat (weekday year month 1) (span [ class "date-picker__pad", attribute "aria-hidden" "true" ] [])
                ++ List.map
                    (\day ->
                        let
                            iso =
                                String.fromInt year ++ "-" ++ pad month ++ "-" ++ pad day
                        in
                        button
                            [ type_ "button"
                            , class "date-picker__day"
                            , attribute "aria-label" (monthName month ++ " " ++ String.fromInt day ++ ", " ++ String.fromInt year)
                            , onClick (ChooseDate target iso)
                            ]
                            [ text (String.fromInt day) ]
                    )
                    (List.range 1 (daysInMonth year month))
            )
        ]


fetchCsrf : Cmd Msg
fetchCsrf =
    Http.get
        { url = "/api/csrf"
        , expect = Http.expectJson GotCsrf (Decode.field "csrfToken" Decode.string)
        }


sendInquiry : String -> Inquiry -> Cmd Msg
sendInquiry token form =
    Http.request
        { method = "POST"
        , headers = [ Http.header "X-CSRF-Token" token ]
        , url = "/api/inquiries"
        , body =
            Http.jsonBody
                (Encode.object
                    [ ( "hotelId", Encode.string form.hotelId )
                    , ( "roomId", Encode.string form.roomId )
                    , ( "offerId", Encode.string form.offerId )
                    , ( "checkIn", Encode.string form.checkIn )
                    , ( "checkOut", Encode.string form.checkOut )
                    , ( "adults", Encode.int (Maybe.withDefault 2 (String.toInt form.adults)) )
                    , ( "children", Encode.int (Maybe.withDefault 0 (String.toInt form.children)) )
                    , ( "name", Encode.string form.guest )
                    , ( "email", Encode.string form.email )
                    , ( "phone", Encode.string form.phone )
                    , ( "message", Encode.string form.message )
                    , ( "website", Encode.string form.website )
                    ]
                )
        , expect = Http.expectStringResponse Submitted readInquiry
        , timeout = Nothing
        , tracker = Nothing
        }


readInquiry : Http.Response String -> Result String String
readInquiry response =
    case response of
        Http.GoodStatus_ _ body ->
            Decode.decodeString (Decode.field "reference" Decode.string) body
                |> Result.mapError (\_ -> "The inquiry could not be confirmed.")

        Http.BadStatus_ _ body ->
            Err body

        _ ->
            Err "The inquiry could not be sent. Please try again."


setInquiry : String -> String -> Inquiry -> Inquiry
setInquiry field next form =
    case field of
        "adults" ->
            { form | adults = next }

        "children" ->
            { form | children = next }

        "name" ->
            { form | guest = next }

        "email" ->
            { form | email = next }

        "phone" ->
            { form | phone = next }

        "message" ->
            { form | message = next }

        "website" ->
            { form | website = next }

        _ ->
            form


decodeSearch : Decode.Value -> SearchForm
decodeSearch value =
    let
        initial =
            Decode.decodeValue (Decode.field "initial" Decode.value) value |> Result.withDefault Encode.null
    in
    { destinations = decodeList destinationDecoder "destinations" value
    , destination = stringField "destination" initial
    , query = stringField "query" initial
    , checkIn = stringField "checkIn" initial
    , checkOut = stringField "checkOut" initial
    , adults = intField "adults" 2 initial
    , children = intField "children" 0 initial
    , compact = Decode.decodeValue (Decode.field "compact" Decode.bool) value |> Result.withDefault False
    , open = Nothing
    , year = 2026
    , month = 9
    }


decodeFilters : Decode.Value -> SearchFilters
decodeFilters value =
    let
        filters =
            Decode.decodeValue (Decode.field "filters" Decode.value) value |> Result.withDefault Encode.null
    in
    { destination = stringField "destination" filters
    , query = stringField "query" filters
    , checkIn = stringField "checkIn" filters
    , checkOut = stringField "checkOut" filters
    , adults = intField "adults" 2 filters
    , children = intField "children" 0 filters
    , minPrice = numberField "minPrice" filters
    , maxPrice = numberField "maxPrice" filters
    , rating = numberField "rating" filters
    , amenities = decodeList amenityDecoder "amenities" value
    , selected = Decode.decodeValue (Decode.field "amenities" (Decode.list Decode.string)) filters |> Result.withDefault []
    , offers = Decode.decodeValue (Decode.field "offers" Decode.bool) filters |> Result.withDefault False
    , sort = stringField "sort" filters
    }


decodeGallery : Decode.Value -> Gallery
decodeGallery value =
    let
        items =
            decodeList photoDecoder "items" value
    in
    { name = stringField "name" value
    , items = items
    , open = False
    , active = List.head items |> Maybe.map .id |> Maybe.withDefault ""
    , category = "All"
    , touch = 0
    }


decodeInquiry : Decode.Value -> Inquiry
decodeInquiry value =
    { hotelId = Decode.decodeValue (Decode.at [ "hotel", "id" ] Decode.string) value |> Result.withDefault ""
    , hotelName = Decode.decodeValue (Decode.at [ "hotel", "name" ] Decode.string) value |> Result.withDefault ""
    , roomId = Decode.decodeValue (Decode.at [ "room", "id" ] Decode.string) value |> Result.withDefault ""
    , offerId = Decode.decodeValue (Decode.at [ "offer", "id" ] Decode.string) value |> Result.withDefault ""
    , checkIn = ""
    , checkOut = ""
    , adults = "2"
    , children = "0"
    , guest = ""
    , email = ""
    , phone = ""
    , message = ""
    , website = ""
    , pending = False
    , error = ""
    , reference = ""
    , open = Nothing
    , year = 2026
    , month = 9
    }


decodeNav : Decode.Value -> MobileNav
decodeNav value =
    { links = decodeList linkDecoder "links" value
    , path = stringField "path" value
    , open = False
    }


destinationDecoder : Decode.Decoder Destination
destinationDecoder =
    Decode.map2 Destination (Decode.field "slug" Decode.string) (Decode.field "name" Decode.string)


amenityDecoder : Decode.Decoder Amenity
amenityDecoder =
    Decode.map2 Amenity (Decode.field "id" Decode.string) (Decode.field "name" Decode.string)


photoDecoder : Decode.Decoder Photo
photoDecoder =
    Decode.map7 Photo
        (Decode.field "id" Decode.string)
        (Decode.field "src" Decode.string)
        (Decode.field "alt" Decode.string)
        (Decode.oneOf [ Decode.field "caption" Decode.string, Decode.succeed "" ])
        (Decode.field "category" Decode.string)
        (Decode.oneOf [ Decode.field "webp" Decode.string, Decode.succeed "" ])
        (Decode.oneOf [ Decode.field "avif" Decode.string, Decode.succeed "" ])


linkDecoder : Decode.Decoder Link
linkDecoder =
    Decode.map3 Link
        (Decode.field "label" Decode.string)
        (Decode.field "href" Decode.string)
        (Decode.oneOf [ Decode.field "description" Decode.string, Decode.succeed "" ])


decodeList : Decode.Decoder a -> String -> Decode.Value -> List a
decodeList decoder field value =
    Decode.decodeValue (Decode.field field (Decode.list decoder)) value |> Result.withDefault []


stringField : String -> Decode.Value -> String
stringField field value =
    Decode.decodeValue (Decode.field field Decode.string) value |> Result.withDefault ""


intField : String -> Int -> Decode.Value -> Int
intField field fallback value =
    Decode.decodeValue (Decode.field field Decode.int) value |> Result.withDefault fallback


numberField : String -> Decode.Value -> String
numberField field value =
    Decode.decodeValue
        (Decode.field field
            (Decode.oneOf
                [ Decode.map String.fromFloat Decode.float
                , Decode.string
                ]
            )
        )
        value
        |> Result.withDefault ""


formField : String -> String -> Html Msg -> Html Msg
formField fieldId labelText control =
    div [ class "search-form__field" ]
        [ label [ class "label", Html.Attributes.for fieldId ] [ text labelText ]
        , control
        ]


labeled : String -> String -> Html Msg -> Html Msg
labeled fieldId labelText control =
    div [ class "field" ]
        [ label [ class "label", Html.Attributes.for fieldId ] [ text labelText ]
        , div [ class "control" ] [ control ]
        ]


hidden : String -> String -> Html msg
hidden fieldName fieldValue =
    input [ type_ "hidden", name fieldName, value fieldValue ] []


preventSubmit : msg -> Html.Attribute msg
preventSubmit msg =
    preventDefaultOn "submit" (Decode.succeed ( msg, True ))


plural : Int -> String
plural count =
    if count == 1 then
        ""

    else
        "s"


activeId : Gallery -> String
activeId gallery =
    List.head gallery.items |> Maybe.map .id |> Maybe.withDefault ""


visiblePhotos : Gallery -> List Photo
visiblePhotos gallery =
    if gallery.category == "All" then
        gallery.items

    else
        List.filter (\photo -> photo.category == gallery.category) gallery.items


categoryNames : List Photo -> List String
categoryNames items =
    List.foldl
        (\photo names ->
            if photo.category == "" || List.member photo.category names then
                names

            else
                names ++ [ photo.category ]
        )
        [ "All" ]
        items


firstPhotoId : List Photo -> String -> String
firstPhotoId items category =
    let
        photos =
            if category == "All" then
                items

            else
                List.filter (\photo -> photo.category == category) items
    in
    List.head photos |> Maybe.map .id |> Maybe.withDefault ""


photoIndex : List Photo -> String -> Int
photoIndex photos active =
    List.indexedMap Tuple.pair photos
        |> List.filter (\( _, photo ) -> photo.id == active)
        |> List.head
        |> Maybe.map Tuple.first
        |> Maybe.withDefault 0


neighbor : List Photo -> String -> Int -> String
neighbor items current delta =
    let
        ids =
            List.map .id items

        index =
            List.indexedMap Tuple.pair ids
                |> List.filter (\( _, id ) -> id == current)
                |> List.head
                |> Maybe.map Tuple.first
                |> Maybe.withDefault 0

        next =
            modBy (max 1 (List.length ids)) (index + delta)
    in
    List.drop next ids |> List.head |> Maybe.withDefault current


shift : Int -> Int -> Int -> ( Int, Int )
shift year month delta =
    let
        total =
            year * 12 + (month - 1) + delta
    in
    ( total // 12, modBy 12 total + 1 )


weekday : Int -> Int -> Int -> Int
weekday year month day =
    let
        offsets =
            [ 0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4 ]

        adjustedYear =
            if month < 3 then
                year - 1

            else
                year

        offset =
            List.drop (month - 1) offsets |> List.head |> Maybe.withDefault 0
    in
    modBy 7 (adjustedYear + (adjustedYear // 4) - (adjustedYear // 100) + (adjustedYear // 400) + offset + day)


daysInMonth : Int -> Int -> Int
daysInMonth year month =
    case month of
        2 ->
            if modBy 4 year == 0 && (modBy 100 year /= 0 || modBy 400 year == 0) then
                29

            else
                28

        4 ->
            30

        6 ->
            30

        9 ->
            30

        11 ->
            30

        _ ->
            31


pad : Int -> String
pad number =
    if number < 10 then
        "0" ++ String.fromInt number

    else
        String.fromInt number


monthName : Int -> String
monthName month =
    case month of
        1 ->
            "January"

        2 ->
            "February"

        3 ->
            "March"

        4 ->
            "April"

        5 ->
            "May"

        6 ->
            "June"

        7 ->
            "July"

        8 ->
            "August"

        9 ->
            "September"

        10 ->
            "October"

        11 ->
            "November"

        _ ->
            "December"
